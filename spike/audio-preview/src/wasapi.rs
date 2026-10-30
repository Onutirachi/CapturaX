//! WASAPI audio preview, started from the previous CapturaX's (CapturaX-old, branch feat/hibrido,
//! src-tauri/src/audio) for comparison with the GStreamer path (T053), f32 stereo 48 kHz inside.
//!
//! - **Shared** side: the shortest period the Windows mixer allows for the device (IAudioClient3,
//!   often a few ms instead of 10), else the normal 10 ms; the output keeps 1.5 periods queued.
//! - **Exclusive** side: skips the Windows mixer, the device's own format, one period per packet;
//!   no other app can use the device meanwhile.
//! - Between the two, the audio waiting in the app is held a small margin above what the output
//!   takes each time: the playback rate follows the level (up to ±1000 ppm, inaudible), so the
//!   two clocks never drift apart and nothing is cut.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, SPEAKER_FRONT_LEFT, SPEAKER_FRONT_RIGHT, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Threading::{AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject};
use windows::core::{Interface, w};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
/// 20 ms WASAPI buffer on a shared side opened the normal way.
const SHARED_BUFFER_HNS: i64 = 200_000;
/// Past this much above what the output needs, the audio waiting in the app is cut (a stall,
/// not drift: drift is followed by the rate).
const CUT_ABOVE: usize = ms(40.0);
/// The rate correction: the error is corrected in about 2 s, never faster than ±1000 ppm.
const CORRECTION_SECONDS: f64 = 2.0;
const MAX_CORRECTION: f64 = 0.001;
const CONTROL_EVERY: Duration = Duration::from_millis(500);

const fn ms(ms: f64) -> usize {
    (RATE as f64 * ms / 1000.0) as usize
}

fn frames_ms(frames: f64) -> f64 {
    frames * 1000.0 / RATE as f64
}

fn hns_ms(hns: i64) -> f64 {
    hns as f64 / 10_000.0
}

/// How each side opens: shared or exclusive, with a period in milliseconds (`None`: the
/// shortest the device or the Windows mixer allows).
#[derive(Clone, Copy, Debug)]
pub struct Side {
    pub exclusive: bool,
    pub period_ms: Option<f64>,
}

struct Event(HANDLE);

impl Event {
    fn new() -> anyhow::Result<Self> {
        Ok(Self(unsafe { CreateEventW(None, false, false, None)? }))
    }

    fn wait(&self, timeout_ms: u32) {
        let _ = unsafe { WaitForSingleObject(self.0, timeout_ms) } == WAIT_OBJECT_0;
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

unsafe impl Send for Event {}

fn com() {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

fn pro_audio() {
    let mut task = 0u32;
    let _ = unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task) };
}

fn friendly_name(device: &IMMDevice) -> String {
    unsafe {
        let Ok(store) = device.OpenPropertyStore(STGM_READ) else { return String::new() };
        let Ok(value) = store.GetValue(&PKEY_Device_FriendlyName) else { return String::new() };
        let Ok(text) = PropVariantToStringAlloc(&value) else { return String::new() };
        let name = text.to_string().unwrap_or_default();
        CoTaskMemFree(Some(text.0 as _));
        name
    }
}

/// The endpoint with this friendly name (as GStreamer lists it), or the default one for `None`.
fn device(flow: EDataFlow, name: Option<&str>) -> anyhow::Result<IMMDevice> {
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let Some(name) = name else { return Ok(unsafe { enumerator.GetDefaultAudioEndpoint(flow, eConsole)? }) };
    let collection = unsafe { enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)? };
    for i in 0..unsafe { collection.GetCount()? } {
        let device = unsafe { collection.Item(i)? };
        if friendly_name(&device) == name {
            return Ok(device);
        }
    }
    anyhow::bail!("no WASAPI endpoint named {name:?}")
}

/// Sample formats, in the order exclusive mode tries them (shared mode always uses f32).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sample {
    F32,
    I32,
    I24In32,
    I24,
    I16,
}

impl Sample {
    const ALL: [Sample; 5] = [Sample::F32, Sample::I32, Sample::I24In32, Sample::I24, Sample::I16];

    fn bytes(self) -> usize {
        match self {
            Sample::F32 | Sample::I32 | Sample::I24In32 => 4,
            Sample::I24 => 3,
            Sample::I16 => 2,
        }
    }

    fn wave_format(self) -> WAVEFORMATEXTENSIBLE {
        let bits = (self.bytes() * 8) as u16;
        let valid = if matches!(self, Sample::I24In32 | Sample::I24) { 24 } else { bits };
        let block_align = (CHANNELS * self.bytes()) as u16;
        WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: CHANNELS as u16,
                nSamplesPerSec: RATE,
                nAvgBytesPerSec: RATE * block_align as u32,
                nBlockAlign: block_align,
                wBitsPerSample: bits,
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: valid },
            dwChannelMask: SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT,
            SubFormat: if self == Sample::F32 { KSDATAFORMAT_SUBTYPE_IEEE_FLOAT } else { KSDATAFORMAT_SUBTYPE_PCM },
        }
    }

    fn read(self, data: &[u8], out: &mut VecDeque<f32>) {
        out.extend(data.chunks_exact(self.bytes()).map(|b| match self {
            Sample::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Sample::I32 | Sample::I24In32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            Sample::I24 => i32::from_le_bytes([0, b[0], b[1], b[2]]) as f32 / 2_147_483_648.0,
            Sample::I16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
        }));
    }

    fn write(self, samples: &[f32], data: &mut [u8]) {
        for (value, b) in samples.iter().zip(data.chunks_exact_mut(self.bytes())) {
            let value = value.clamp(-1.0, 1.0);
            let int = (value as f64 * 2_147_483_647.0) as i32;
            match self {
                Sample::F32 => b.copy_from_slice(&value.to_le_bytes()),
                Sample::I32 | Sample::I24In32 => b.copy_from_slice(&int.to_le_bytes()),
                Sample::I24 => b.copy_from_slice(&int.to_le_bytes()[1..]),
                Sample::I16 => b.copy_from_slice(&((value * 32_767.0) as i16).to_le_bytes()),
            }
        }
    }
}

struct Opened {
    client: IAudioClient,
    sample: Sample,
    /// WASAPI buffer and period, in frames.
    frames: u32,
    period: u32,
    how: &'static str,
}

/// The device's mix format, if it is the f32 stereo 48 kHz the app uses (the low-latency shared
/// stream takes no format conversion).
fn mix_is_ours(client: &IAudioClient3) -> anyhow::Result<bool> {
    unsafe {
        let mix = client.GetMixFormat()?;
        let format = *mix;
        let float = match format.wFormatTag as u32 {
            WAVE_FORMAT_IEEE_FLOAT => true,
            WAVE_FORMAT_EXTENSIBLE => {
                let subformat = std::ptr::addr_of!((*(mix as *const WAVEFORMATEXTENSIBLE)).SubFormat).read_unaligned();
                subformat == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            }
            _ => false,
        };
        CoTaskMemFree(Some(mix as _));
        Ok(float && format.nSamplesPerSec == RATE && format.nChannels as usize == CHANNELS && format.wBitsPerSample == 32)
    }
}

fn open_shared(device: &IMMDevice, event: &Event, side: Side) -> anyhow::Result<Opened> {
    // The shortest period the Windows mixer allows, when the device's mix format is ours.
    let client3: IAudioClient3 = unsafe { device.Activate(CLSCTX_ALL, None)? };
    if mix_is_ours(&client3)? {
        let format = Sample::F32.wave_format();
        let (mut default, mut fundamental, mut min, mut max) = (0u32, 0u32, 0u32, 0u32);
        unsafe { client3.GetSharedModeEnginePeriod(&format.Format, &mut default, &mut fundamental, &mut min, &mut max)? };
        let wanted = side.period_ms.map_or(min, |ms| ms_frames(ms));
        let steps = (wanted.clamp(min, max) - min + fundamental / 2) / fundamental.max(1);
        let period = (min + steps * fundamental).min(max);
        unsafe {
            client3.InitializeSharedAudioStream(AUDCLNT_STREAMFLAGS_EVENTCALLBACK, period, &format.Format, None)?;
            client3.SetEventHandle(event.0)?;
        }
        let frames = unsafe { client3.GetBufferSize()? };
        return Ok(Opened { client: client3.cast()?, sample: Sample::F32, frames, period, how: "shared, short period" });
    }

    let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let format = Sample::F32.wave_format();
    let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    unsafe {
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, SHARED_BUFFER_HNS, 0, &format.Format, None)?;
        client.SetEventHandle(event.0)?;
    }
    let mut default = 0i64;
    unsafe { client.GetDevicePeriod(Some(&mut default), None)? };
    let frames = unsafe { client.GetBufferSize()? };
    let period = ms_frames(hns_ms(default));
    Ok(Opened { client, sample: Sample::F32, frames, period, how: "shared, converted by Windows" })
}

fn ms_frames(ms: f64) -> u32 {
    (RATE as f64 * ms / 1000.0).round() as u32
}

fn open_exclusive(device: &IMMDevice, event: &Event, side: Side) -> anyhow::Result<Opened> {
    let probe: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let sample = Sample::ALL
        .into_iter()
        .find(|s| unsafe { probe.IsFormatSupported(AUDCLNT_SHAREMODE_EXCLUSIVE, &s.wave_format().Format, None) }.is_ok())
        .ok_or_else(|| anyhow::anyhow!("the device takes none of the 48 kHz stereo formats in exclusive mode"))?;
    let (mut default, mut minimum) = (0i64, 0i64);
    unsafe { probe.GetDevicePeriod(Some(&mut default), Some(&mut minimum))? };
    let mut period = side.period_ms.map_or(minimum, |ms| ((ms * 10_000.0) as i64).max(minimum));
    let format = sample.wave_format();
    // In exclusive event mode the buffer is one period; a period the device cannot align is
    // retried with the size it proposes.
    for _ in 0..2 {
        let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
        match unsafe { client.Initialize(AUDCLNT_SHAREMODE_EXCLUSIVE, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, period, period, &format.Format, None) } {
            Ok(()) => {
                unsafe { client.SetEventHandle(event.0)? };
                let frames = unsafe { client.GetBufferSize()? };
                return Ok(Opened { client, sample, frames, period: frames, how: "exclusive" });
            }
            Err(error) if error.code() == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED => {
                let frames = unsafe { client.GetBufferSize()? };
                period = (10_000_000.0 * frames as f64 / RATE as f64 + 0.5) as i64;
            }
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("the device refused every exclusive period")
}

fn open(label: &str, device: &IMMDevice, event: &Event, side: Side) -> anyhow::Result<Opened> {
    let opened = if side.exclusive { open_exclusive(device, event, side)? } else { open_shared(device, event, side)? };
    let latency = unsafe { opened.client.GetStreamLatency()? };
    println!(
        "{label}: {} {:?}, period {:.2} ms, buffer {:.2} ms, stream latency {:.1} ms",
        opened.how,
        opened.sample,
        frames_ms(opened.period as f64),
        frames_ms(opened.frames as f64),
        hns_ms(latency)
    );
    Ok(opened)
}

#[derive(Default)]
struct Stats {
    captured: AtomicU64,
}

fn capture(name: String, side: Side, ring: Arc<Mutex<VecDeque<f32>>>, stats: Arc<Stats>, ready: std::sync::mpsc::Sender<anyhow::Result<u32>>) {
    com();
    pro_audio();
    let setup = || -> anyhow::Result<(Opened, IAudioCaptureClient, Event)> {
        let event = Event::new()?;
        let opened = open("input ", &device(eCapture, Some(&name))?, &event, side)?;
        let capture: IAudioCaptureClient = unsafe { opened.client.GetService()? };
        unsafe { opened.client.Start()? };
        Ok((opened, capture, event))
    };
    let (opened, capture, event) = match setup() {
        Ok(parts) => {
            let _ = ready.send(Ok(parts.0.period));
            parts
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut scratch = VecDeque::new();
    loop {
        event.wait(100);
        loop {
            let Ok(available) = (unsafe { capture.GetNextPacketSize() }) else { return };
            if available == 0 {
                break;
            }
            let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
            if unsafe { capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None) }.is_err() {
                return;
            }
            stats.captured.fetch_add(frames as u64, Ordering::Relaxed);
            let samples = frames as usize * CHANNELS;
            scratch.clear();
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                scratch.resize(samples, 0.0);
            } else {
                let bytes = unsafe { std::slice::from_raw_parts(data, samples * opened.sample.bytes()) };
                opened.sample.read(bytes, &mut scratch);
            }
            let _ = unsafe { capture.ReleaseBuffer(frames) };
            ring.lock().unwrap().extend(scratch.drain(..));
        }
    }
}

/// Reads the waiting audio at a rate slightly off 1 (linear interpolation), so that the lowest
/// level seen over each half second stays `margin` frames above what the output needs.
struct Follower {
    ratio: f64,
    /// Position of the next output frame from the first waiting frame.
    position: f64,
    margin: f64,
    lowest: f64,
    since: Instant,
}

impl Follower {
    /// Waiting frames needed to produce `frames` output frames.
    fn needed(&self, frames: usize) -> usize {
        (self.position + frames as f64 * self.ratio).floor() as usize + 2
    }

    fn produce(&mut self, ring: &mut VecDeque<f32>, frames: usize, out: &mut Vec<f32>) {
        for _ in 0..frames {
            let i = self.position.floor() as usize;
            let t = (self.position - i as f64) as f32;
            for c in 0..CHANNELS {
                let (a, b) = (ring[i * CHANNELS + c], ring[(i + 1) * CHANNELS + c]);
                out.push(a + (b - a) * t);
            }
            self.position += self.ratio;
        }
        let used = self.position.floor() as usize;
        ring.drain(..used * CHANNELS);
        self.position -= used as f64;
    }

    /// Records the spare frames of one output event; every half second, sets the rate from the
    /// lowest of them.
    fn observe(&mut self, spare: f64) {
        self.lowest = self.lowest.min(spare);
        if self.since.elapsed() >= CONTROL_EVERY {
            let error = self.lowest - self.margin;
            self.ratio = 1.0 + (error / (CORRECTION_SECONDS * RATE as f64)).clamp(-MAX_CORRECTION, MAX_CORRECTION);
            self.lowest = f64::MAX;
            self.since = Instant::now();
        }
    }
}

/// Plays `input` on `output` (system default for `None`) until the process is stopped, with
/// `margin_ms` of spare audio in the app; with `stats`, prints every 10 s what waits where.
pub fn run(input: &str, output: Option<&str>, sides: (Side, Side), margin_ms: f64, stats_every: Option<Duration>) -> anyhow::Result<()> {
    com();
    pro_audio();
    let (input_side, output_side) = sides;
    let ring = Arc::new(Mutex::new(VecDeque::<f32>::new()));
    let stats = Arc::new(Stats::default());
    let (ready, started) = std::sync::mpsc::channel();
    {
        let (name, ring, stats) = (input.to_string(), ring.clone(), stats.clone());
        std::thread::Builder::new().name("wasapi-capture".into()).spawn(move || capture(name, input_side, ring, stats, ready))?;
    }
    let input_period = started.recv()??;

    let event = Event::new()?;
    let opened = open("output", &device(eRender, output)?, &event, output_side)?;
    let client = &opened.client;
    let render: IAudioRenderClient = unsafe { client.GetService()? };
    // Exclusive: every event wants one whole period. Shared: keep 1.5 periods queued in Windows.
    let target = if output_side.exclusive { opened.frames } else { (opened.period * 3 / 2).min(opened.frames) };
    unsafe { client.Start()? };
    println!(
        "Playing (WASAPI). Waiting in the app: what the output takes plus up to one input period ({:.1} ms) plus {margin_ms} ms. Ctrl+C quits.",
        frames_ms(input_period as f64)
    );

    let margin = ms(margin_ms) as f64;
    let mut follower = Follower { ratio: 1.0, position: 0.0, margin, lowest: f64::MAX, since: Instant::now() };
    let mut primed = false;
    let mut chunk = Vec::<f32>::new();
    let (mut cut, mut silent, mut written) = (0u64, 0u64, 0u64);
    let (mut padding_sum, mut padding_max, mut level_sum, mut level_max, mut events) = (0u64, 0u32, 0u64, 0usize, 0u64);
    let (mut ratio_sum, mut lowest_spare) = (0f64, f64::MAX);
    let started = Instant::now();
    let mut last = Instant::now();
    loop {
        event.wait(100);
        let padding = if output_side.exclusive { 0 } else { unsafe { client.GetCurrentPadding()? } };
        let frames = target.saturating_sub(padding) as usize;
        if frames > 0 {
            let mut ring = ring.lock().unwrap();
            let level = ring.len() / CHANNELS;
            let needed = follower.needed(frames);
            // A stall left far more than the margin: drop the oldest instead of playing it late.
            if level > needed + CUT_ABOVE {
                let drop = level - needed - margin as usize;
                ring.drain(..drop * CHANNELS);
                cut += drop as u64;
            }
            let level = ring.len() / CHANNELS;
            let start = needed + input_period as usize + margin as usize;
            if !primed && level >= start {
                // What piled up while the output opened would only be played late.
                ring.drain(..(level - start) * CHANNELS);
                primed = true;
                follower.position = 0.0;
                follower.lowest = f64::MAX;
                follower.since = Instant::now();
            }
            chunk.clear();
            if primed && level >= needed {
                let spare = (level - needed) as f64;
                follower.observe(spare);
                lowest_spare = lowest_spare.min(spare);
                follower.produce(&mut ring, frames, &mut chunk);
            } else {
                primed = false;
                silent += frames as u64;
                chunk.resize(frames * CHANNELS, 0.0);
            }
            drop(ring);
            (padding_sum, padding_max) = (padding_sum + padding as u64, padding_max.max(padding));
            (level_sum, level_max, events) = (level_sum + level as u64, level_max.max(level), events + 1);
            ratio_sum += follower.ratio;
            unsafe {
                let data = render.GetBuffer(frames as u32)?;
                let bytes = std::slice::from_raw_parts_mut(data, chunk.len() * opened.sample.bytes());
                opened.sample.write(&chunk, bytes);
                render.ReleaseBuffer(frames as u32, 0)?;
            }
            written += frames as u64;
        }

        if let Some(every) = stats_every
            && last.elapsed() >= every
        {
            last = Instant::now();
            let n = events.max(1) as f64;
            let elapsed = started.elapsed().as_secs_f64();
            let rate_ppm = |frames: u64| (frames as f64 / (elapsed * RATE as f64) - 1.0) * 1e6;
            println!(
                "[wasapi] {:>5.0} s | in the app {:.1} ms avg, {:.1} max, spare {:.1} ms min | windows queue {:.1} ms avg, {:.1} max | rate {:+.0} ppm | cut {:.0} ms, silence {:.0} ms | input {:+.0} ppm, output {:+.0} ppm",
                elapsed,
                frames_ms(level_sum as f64 / n),
                frames_ms(level_max as f64),
                frames_ms(if lowest_spare == f64::MAX { 0.0 } else { lowest_spare }),
                frames_ms(padding_sum as f64 / n),
                frames_ms(padding_max as f64),
                (ratio_sum / n - 1.0) * 1e6,
                frames_ms(cut as f64),
                frames_ms(silent as f64),
                rate_ppm(stats.captured.load(Ordering::Relaxed)),
                rate_ppm(written),
            );
            (padding_sum, padding_max, level_sum, level_max, events) = (0, 0, 0, 0, 0);
            (ratio_sum, lowest_spare) = (0.0, f64::MAX);
        }
    }
}
