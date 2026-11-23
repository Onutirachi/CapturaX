//! The audio session on Windows (research R2, R7), ported from the spike's WASAPI engine.
//!
//! Each side runs on its own thread (`audio-in`, `audio-out`) in the multithreaded COM apartment,
//! registered with MMCSS "Pro Audio", waiting on its WASAPI event with a 100 ms timeout. Every
//! wake checks the session's stop flag and the side's reopen requests, so nothing the app asks
//! waits for a device: close and reopen are seen within 100 ms. Every outcome is an event.
//!
//! - **Shared**: the endpoint's mix format, any rate and channel count. `IAudioClient3` with the
//!   shortest period the Windows mixer allows; else the normal `Initialize` with a 20 ms buffer.
//!   The output keeps 1.5 periods queued in Windows, or its whole buffer on a period of 10 ms or
//!   more, where a device can take two periods at once (FR-003b, research R17).
//! - **Exclusive**: the first format the device takes (samples × rates × channels, research R2),
//!   its minimum period, one period per event. In use or no format → the side opens shared
//!   and reports `SharedAfterInUse`.
//!
//! The output, shared or exclusive, reopens itself at the first quiet moment once it has played
//! 5 minutes (FR-003a, research R16: a delay grows after it).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HANDLE, S_OK};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED, AUDCLNT_E_CPUUSAGE_EXCEEDED,
    AUDCLNT_E_DEVICE_IN_USE, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_ENDPOINT_CREATE_FAILED,
    AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED, AUDCLNT_E_RESOURCES_INVALIDATED, AUDCLNT_E_SERVICE_NOT_RUNNING,
    AUDCLNT_E_UNSUPPORTED_FORMAT, AUDCLNT_SHAREMODE_EXCLUSIVE, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_STATE_ACTIVE, IAudioCaptureClient, IAudioClient, IAudioClient3,
    IAudioRenderClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject,
};
use windows::core::{HRESULT, HSTRING, Interface, w};

use super::{Com, format};
use crate::audio::{AudioError, Format, OnSessionEvent, Samples, SessionEvent, SessionRequest, Side, StreamMode};

/// How long a side waits for its device before checking the stop flag and its requests.
const WAIT_MS: u32 = 100;
/// The WASAPI buffer of a shared side opened the normal way: 20 ms.
const SHARED_BUFFER_HNS: i64 = 200_000;
/// The rates exclusive mode tries after the other side's.
const EXCLUSIVE_RATES: [u32; 3] = [48_000, 44_100, 96_000];
/// `HRESULT_FROM_WIN32(ERROR_NOT_FOUND)`: no endpoint with that id.
const NOT_FOUND: HRESULT = HRESULT(0x8007_0490_u32 as i32);

/// What a side opens.
struct Open {
    /// The endpoint id; None is the default output.
    device: Option<String>,
    exclusive: bool,
}

/// What both sides share.
struct Shared {
    number: u64,
    samples: Arc<dyn Samples>,
    on_event: OnSessionEvent,
    stop: AtomicBool,
    /// Sides whose thread has not ended; the last one posts `Closed`.
    running: AtomicUsize,
    /// The rate each side runs at (0: not open), for the other side's exclusive probe.
    rates: [AtomicU32; 2],
}

impl Shared {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    fn error(&self, side: Side, error: AudioError) {
        log::warn!("audio {side:?}: {error}");
        (self.on_event)(SessionEvent::Error {
            number: self.number,
            side,
            error,
        });
    }

    /// A side's thread ended; the last one reports the session closed.
    fn finished(&self) {
        if self.running.fetch_sub(1, Ordering::AcqRel) == 1 {
            (self.on_event)(SessionEvent::Closed { number: self.number });
        }
    }

    fn rate(&self, side: Side) -> &AtomicU32 {
        &self.rates[match side {
            Side::Input => 0,
            Side::Output => 1,
        }]
    }
}

pub struct Session {
    shared: Arc<Shared>,
    /// None for an output-only session.
    input: Option<Sender<Open>>,
    output: Sender<Open>,
}

impl Session {
    pub fn open(request: SessionRequest, samples: Arc<dyn Samples>, on_event: OnSessionEvent) -> Self {
        let shared = Arc::new(Shared {
            number: request.number,
            samples,
            on_event,
            stop: AtomicBool::new(false),
            running: AtomicUsize::new(if request.input.is_some() { 2 } else { 1 }),
            rates: [AtomicU32::new(0), AtomicU32::new(0)],
        });
        let input = request.input.map(|input| {
            spawn(
                Side::Input,
                Open {
                    device: Some(input),
                    exclusive: request.input_exclusive,
                },
                &shared,
            )
        });
        let output = spawn(
            Side::Output,
            Open {
                device: request.output,
                exclusive: request.output_exclusive,
            },
            &shared,
        );
        Self { shared, input, output }
    }

    /// Ignored by an output-only session.
    pub fn reopen_input(&self, input: String, exclusive: bool) {
        if let Some(sender) = &self.input {
            let _ = sender.send(Open {
                device: Some(input),
                exclusive,
            });
        }
    }

    pub fn reopen_output(&self, output: Option<String>, exclusive: bool) {
        let _ = self.output.send(Open {
            device: output,
            exclusive,
        });
    }

    pub fn close(&self) {
        self.shared.stop.store(true, Ordering::Release);
    }
}

/// Starts a side's thread; when it cannot start, the side's error is posted from here.
fn spawn(side: Side, first: Open, shared: &Arc<Shared>) -> Sender<Open> {
    let (sender, requests) = mpsc::channel();
    let name = match side {
        Side::Input => "audio-in",
        Side::Output => "audio-out",
    };
    let thread_shared = Arc::clone(shared);
    let started = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || run(side, first, &requests, &thread_shared));
    if let Err(error) = started {
        shared.error(side, AudioError::Failed(format!("{name} thread: {error}")));
        shared.finished();
    }
    sender
}

/// What ends a running stream.
enum Next {
    Reopen(Open),
    Stop,
    Failed(AudioError),
}

fn run(side: Side, first: Open, requests: &Receiver<Open>, shared: &Shared) {
    {
        let _com = Com::init();
        let _pro_audio = ProAudio::register();
        let mut next = Some(first);
        while let Some(open) = next.take() {
            if shared.stopped() {
                break;
            }
            next = match Stream::open(side, &open, shared) {
                Ok(stream) => {
                    shared.rate(side).store(stream.format.rate, Ordering::Release);
                    stream.report(shared);
                    let end = stream.run(requests, shared);
                    drop(stream);
                    shared.rate(side).store(0, Ordering::Release);
                    match end {
                        Next::Reopen(open) => Some(open),
                        Next::Stop => None,
                        Next::Failed(error) => {
                            shared.error(side, error);
                            idle(requests, shared)
                        }
                    }
                }
                Err(error) => {
                    shared.error(side, error);
                    idle(requests, shared)
                }
            };
        }
    }
    shared.finished();
}

/// A side without a stream (after an error) waits for a reopen or the stop.
fn idle(requests: &Receiver<Open>, shared: &Shared) -> Option<Open> {
    loop {
        if shared.stopped() {
            return None;
        }
        match requests.recv_timeout(Duration::from_millis(WAIT_MS.into())) {
            Ok(open) => return Some(latest(open, requests)),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// The last of the waiting requests: only the newest one is opened.
fn latest(mut open: Open, requests: &Receiver<Open>) -> Open {
    while let Ok(newer) = requests.try_recv() {
        open = newer;
    }
    open
}

/// What a running stream checks at each wake.
fn control(requests: &Receiver<Open>, shared: &Shared) -> Option<Next> {
    if shared.stopped() {
        return Some(Next::Stop);
    }
    match requests.try_recv() {
        Ok(open) => Some(Next::Reopen(latest(open, requests))),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Next::Stop),
    }
}

/// The platform error of a WASAPI failure (research R7). `exclusive`: during an exclusive open,
/// where an unsupported format makes the side open shared instead. An invalidated device is
/// `Gone` here; [`taken`] tells it from a device another app took.
fn error(side: Side, exclusive: bool, error: &windows::core::Error) -> AudioError {
    let code = error.code();
    if code == AUDCLNT_E_DEVICE_INVALIDATED || code == NOT_FOUND {
        AudioError::Gone
    } else if code == AUDCLNT_E_DEVICE_IN_USE || code == AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED {
        AudioError::InUse
    } else if exclusive && code == AUDCLNT_E_UNSUPPORTED_FORMAT {
        AudioError::Format
    } else if side == Side::Input && code == E_ACCESSDENIED {
        AudioError::Blocked
    } else {
        AudioError::Failed(text(error))
    }
}

/// The system's text of an error; WASAPI's own codes have none, so they are named.
fn text(error: &windows::core::Error) -> String {
    let code = error.code();
    let names = [
        (AUDCLNT_E_DEVICE_INVALIDATED, "AUDCLNT_E_DEVICE_INVALIDATED"),
        (AUDCLNT_E_DEVICE_IN_USE, "AUDCLNT_E_DEVICE_IN_USE"),
        (
            AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED,
            "AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED",
        ),
        (AUDCLNT_E_UNSUPPORTED_FORMAT, "AUDCLNT_E_UNSUPPORTED_FORMAT"),
        (AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED, "AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED"),
        (AUDCLNT_E_SERVICE_NOT_RUNNING, "AUDCLNT_E_SERVICE_NOT_RUNNING"),
        (AUDCLNT_E_ENDPOINT_CREATE_FAILED, "AUDCLNT_E_ENDPOINT_CREATE_FAILED"),
        (AUDCLNT_E_CPUUSAGE_EXCEEDED, "AUDCLNT_E_CPUUSAGE_EXCEEDED"),
        (AUDCLNT_E_RESOURCES_INVALIDATED, "AUDCLNT_E_RESOURCES_INVALIDATED"),
    ];
    let message = error.message();
    match names.iter().find(|(c, _)| *c == code) {
        Some((_, name)) => format!("{name} ({:#010X})", code.0),
        None if message.is_empty() => format!("{:#010X}", code.0),
        None => format!("{message} ({:#010X})", code.0),
    }
}

/// The thread's MMCSS registration, reverted when dropped.
struct ProAudio(Option<HANDLE>);

impl ProAudio {
    fn register() -> Self {
        let mut task = 0u32;
        // SAFETY: a constant task name and a local index.
        match unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &raw mut task) } {
            Ok(handle) => Self(Some(handle)),
            Err(error) => {
                log::warn!("audio: MMCSS Pro Audio: {error}");
                Self(None)
            }
        }
    }
}

impl Drop for ProAudio {
    fn drop(&mut self) {
        if let Some(handle) = self.0 {
            // SAFETY: the handle registered on this thread.
            let _ = unsafe { AvRevertMmThreadCharacteristics(handle) };
        }
    }
}

/// The auto-reset event WASAPI signals when a period is ready.
struct Event(HANDLE);

impl Event {
    fn new() -> Result<Self, AudioError> {
        // SAFETY: an unnamed auto-reset event, closed on drop.
        unsafe { CreateEventW(None, false, false, None) }
            .map(Self)
            .map_err(|e| AudioError::Failed(e.to_string()))
    }

    fn wait(&self) {
        // SAFETY: the handle lives as long as self.
        let _ = unsafe { WaitForSingleObject(self.0, WAIT_MS) };
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: the handle created in `new`, closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// The endpoint's mix format, freed when dropped.
struct Mix(*mut WAVEFORMATEX);

impl Mix {
    fn of(client: &IAudioClient) -> windows::core::Result<Self> {
        // SAFETY: a COM call on this thread; the pointer is WASAPI's allocation, owned here.
        unsafe { client.GetMixFormat() }.map(Self)
    }

    fn format(&self) -> Option<Format> {
        // SAFETY: WASAPI's whole format.
        unsafe { format::format_of(self.0) }
    }

    fn channels_and_mask(&self) -> (u16, u32) {
        // SAFETY: WASAPI's whole format; read unaligned (packed struct).
        unsafe { (self.0.read_unaligned().nChannels, format::mask_of(self.0)) }
    }
}

impl Drop for Mix {
    fn drop(&mut self) {
        // SAFETY: allocated by GetMixFormat with CoTaskMemAlloc, freed once.
        unsafe { CoTaskMemFree(Some(self.0 as _)) };
    }
}

/// The endpoint with this id, active; None is the default output.
fn device(side: Side, id: Option<&str>) -> Result<IMMDevice, AudioError> {
    let failed = |e: windows::core::Error| error(side, false, &e);
    // SAFETY: COM calls on this side's thread, COM initialized; `id` outlives the call.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(failed)?;
        match id {
            None => enumerator.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| {
                if e.code() == NOT_FOUND {
                    AudioError::Failed("no default output".into())
                } else {
                    failed(e)
                }
            }),
            Some(id) => {
                let device = enumerator.GetDevice(&HSTRING::from(id)).map_err(failed)?;
                if device.GetState().map_err(failed)? == DEVICE_STATE_ACTIVE {
                    Ok(device)
                } else {
                    Err(AudioError::Gone)
                }
            }
        }
    }
}

/// An initialized audio client, before its service and start.
struct Opened {
    client: IAudioClient,
    format: Format,
    /// The WASAPI buffer and the period, in frames.
    buffer: u32,
    period: u32,
}

fn hns_frames(hns: i64, rate: u32) -> u32 {
    (hns as f64 * f64::from(rate) / 10_000_000.0).round() as u32
}

/// Shared: the mix format, the shortest period through `IAudioClient3`, else a 20 ms buffer.
fn open_shared(side: Side, device: &IMMDevice, event: &Event) -> Result<Opened, AudioError> {
    let failed = |e: windows::core::Error| error(side, false, &e);
    // SAFETY: COM calls on this side's thread; the mix format outlives every call that reads it;
    // the event outlives the client (the stream drops it last).
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(failed)?;
        let mix = Mix::of(&client).map_err(failed)?;
        let format = mix.format().ok_or(AudioError::Format)?;

        if let Ok(client3) = client.cast::<IAudioClient3>() {
            let (mut default, mut fundamental, mut min, mut max) = (0, 0, 0, 0);
            let short = client3
                .GetSharedModeEnginePeriod(
                    mix.0,
                    &raw mut default,
                    &raw mut fundamental,
                    &raw mut min,
                    &raw mut max,
                )
                .and_then(|()| {
                    client3.InitializeSharedAudioStream(AUDCLNT_STREAMFLAGS_EVENTCALLBACK, min, mix.0, None)
                });
            match short {
                Ok(()) => {
                    client.SetEventHandle(event.0).map_err(failed)?;
                    let buffer = client.GetBufferSize().map_err(failed)?;
                    return Ok(Opened {
                        client,
                        format,
                        buffer,
                        period: min,
                    });
                }
                Err(e) => log::info!("audio {side:?}: short shared period refused ({e}); 20 ms buffer"),
            }
        }

        // A client that failed to initialize cannot be initialized again: a new one.
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(failed)?;
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                SHARED_BUFFER_HNS,
                0,
                mix.0,
                None,
            )
            .map_err(failed)?;
        client.SetEventHandle(event.0).map_err(failed)?;
        let mut default = 0i64;
        client.GetDevicePeriod(Some(&raw mut default), None).map_err(failed)?;
        let buffer = client.GetBufferSize().map_err(failed)?;
        Ok(Opened {
            client,
            format,
            buffer,
            period: hns_frames(default, format.rate),
        })
    }
}

/// Exclusive: the first format the device takes, its minimum period, one period per buffer.
fn open_exclusive(
    side: Side,
    device: &IMMDevice,
    event: &Event,
    other_rate: Option<u32>,
) -> Result<Opened, AudioError> {
    let failed = |e: windows::core::Error| error(side, true, &e);
    // SAFETY: COM calls on this side's thread; each wave format outlives the calls that read it;
    // the event outlives the client.
    unsafe {
        let probe: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(failed)?;
        let (mix_channels, mix_mask) = Mix::of(&probe).map_err(failed)?.channels_and_mask();
        let mut rates = Vec::new();
        for rate in other_rate.into_iter().chain(EXCLUSIVE_RATES) {
            if !rates.contains(&rate) {
                rates.push(rate);
            }
        }
        let mut channels = vec![2];
        if mix_channels != 2 {
            channels.push(mix_channels);
        }

        let mut found = None;
        'search: for sample in format::EXCLUSIVE_SAMPLES {
            for &rate in &rates {
                for &count in &channels {
                    let format = Format {
                        rate,
                        channels: count,
                        sample,
                    };
                    let mask = if count == mix_channels {
                        mix_mask
                    } else {
                        format::standard_mask(count)
                    };
                    let wave = format::wave_format(format, mask);
                    let result = probe.IsFormatSupported(AUDCLNT_SHAREMODE_EXCLUSIVE, (&raw const wave).cast(), None);
                    if result == S_OK {
                        found = Some((format, wave));
                        break 'search;
                    }
                    // In use, not allowed or gone: no other format will do. Anything else is
                    // this format refused (drivers differ in the code they give).
                    if [
                        AUDCLNT_E_DEVICE_IN_USE,
                        AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED,
                        AUDCLNT_E_DEVICE_INVALIDATED,
                    ]
                    .contains(&result)
                    {
                        return Err(failed(result.into()));
                    }
                }
            }
        }
        let (format, wave) = found.ok_or(AudioError::Format)?;

        let mut minimum = 0i64;
        probe.GetDevicePeriod(None, Some(&raw mut minimum)).map_err(failed)?;
        let mut period = minimum;
        // In exclusive event mode the buffer is one period; a period the device cannot align is
        // retried with the size it proposes (the spike's rule).
        for _ in 0..2 {
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(failed)?;
            let initialized = client.Initialize(
                AUDCLNT_SHAREMODE_EXCLUSIVE,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                period,
                period,
                (&raw const wave).cast(),
                None,
            );
            match initialized {
                Ok(()) => {
                    client.SetEventHandle(event.0).map_err(failed)?;
                    let buffer = client.GetBufferSize().map_err(failed)?;
                    return Ok(Opened {
                        client,
                        format,
                        buffer,
                        period: buffer,
                    });
                }
                Err(e) if e.code() == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED => {
                    let frames = client.GetBufferSize().map_err(failed)?;
                    period = (10_000_000.0 * f64::from(frames) / f64::from(format.rate)).round() as i64;
                }
                Err(e) => return Err(failed(e)),
            }
        }
        Err(AudioError::Failed("the device refused every exclusive period".into()))
    }
}

/// The side's WASAPI service.
enum Io {
    Capture(IAudioCaptureClient),
    Render(IAudioRenderClient),
}

/// `Gone` from a device whose endpoint is still active is `InUse`: another app took it in
/// exclusive mode (measured: the shared streams on it are invalidated, the endpoint stays).
fn taken(error: AudioError, device: &IMMDevice) -> AudioError {
    // SAFETY: a COM call on this side's thread.
    if error == AudioError::Gone && unsafe { device.GetState() }.is_ok_and(|s| s == DEVICE_STATE_ACTIVE) {
        AudioError::InUse
    } else {
        error
    }
}

/// A started side. Fields drop in order: the service, the client, then the event it signals.
struct Stream {
    /// What it opened, for its own reopen (FR-003a).
    endpoint: Option<String>,
    exclusive: bool,
    io: Io,
    client: IAudioClient,
    event: Event,
    device: IMMDevice,
    side: Side,
    mode: StreamMode,
    format: Format,
    buffer: u32,
    period: u32,
}

impl Stream {
    fn open(side: Side, open: &Open, shared: &Shared) -> Result<Self, AudioError> {
        let device = device(side, open.device.as_deref())?;
        Self::start(side, open, shared, device.clone()).map_err(|e| taken(e, &device))
    }

    fn start(side: Side, open: &Open, shared: &Shared, device: IMMDevice) -> Result<Self, AudioError> {
        let event = Event::new()?;
        let (opened, mode) = if open.exclusive {
            let other = shared.rate(match side {
                Side::Input => Side::Output,
                Side::Output => Side::Input,
            });
            let other_rate = Some(other.load(Ordering::Acquire)).filter(|&r| r != 0);
            match open_exclusive(side, &device, &event, other_rate) {
                Ok(opened) => (opened, StreamMode::Exclusive),
                Err(refused @ (AudioError::InUse | AudioError::Format)) => {
                    log::info!("audio {side:?}: exclusive refused ({refused}); shared");
                    (open_shared(side, &device, &event)?, StreamMode::SharedAfterInUse)
                }
                Err(other) => return Err(other),
            }
        } else {
            (open_shared(side, &device, &event)?, StreamMode::Shared)
        };
        let failed = |e: windows::core::Error| error(side, false, &e);
        // SAFETY: COM calls on this side's thread on the client just initialized.
        let io = unsafe {
            match side {
                Side::Input => Io::Capture(opened.client.GetService().map_err(failed)?),
                Side::Output => Io::Render(opened.client.GetService().map_err(failed)?),
            }
        };
        // SAFETY: as above.
        unsafe { opened.client.Start() }.map_err(failed)?;
        Ok(Self {
            endpoint: open.device.clone(),
            exclusive: open.exclusive,
            io,
            client: opened.client,
            event,
            device,
            side,
            mode,
            format: opened.format,
            buffer: opened.buffer,
            period: opened.period,
        })
    }

    fn report(&self, shared: &Shared) {
        // SAFETY: a COM call on this side's thread. A shared stream through `IAudioClient3`
        // reports 0 (measured): it does not say.
        let latency = unsafe { self.client.GetStreamLatency() }.ok().filter(|&hns| hns > 0);
        let event = SessionEvent::Opened {
            number: shared.number,
            side: self.side,
            mode: self.mode,
            format: self.format,
            period_ms: f64::from(self.period) * 1000.0 / f64::from(self.format.rate),
            reported_latency_ms: latency.map(|hns| hns as f64 / 10_000.0),
        };
        log::info!("audio {:?}: {event:?}", self.side);
        (shared.on_event)(event);
    }

    fn failed(&self, e: &windows::core::Error) -> Next {
        Next::Failed(taken(error(self.side, false, e), &self.device))
    }

    /// Moves samples until a reopen, the stop or an error.
    fn run(&self, requests: &Receiver<Open>, shared: &Shared) -> Next {
        match &self.io {
            Io::Capture(capture) => self.capture(capture, requests, shared),
            Io::Render(render) => self.render(render, requests, shared),
        }
    }

    fn capture(&self, capture: &IAudioCaptureClient, requests: &Receiver<Open>, shared: &Shared) -> Next {
        let sample_bytes = format::bytes(self.format.sample);
        let channels = usize::from(self.format.channels);
        let mut samples = Vec::new();
        loop {
            self.event.wait();
            if let Some(next) = control(requests, shared) {
                return next;
            }
            loop {
                // SAFETY: COM calls on this side's thread; the packet is read before it is
                // released.
                unsafe {
                    let available = match capture.GetNextPacketSize() {
                        Ok(frames) => frames,
                        Err(e) => return self.failed(&e),
                    };
                    if available == 0 {
                        break;
                    }
                    let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                    if let Err(e) = capture.GetBuffer(&raw mut data, &raw mut frames, &raw mut flags, None, None) {
                        return self.failed(&e);
                    }
                    let count = frames as usize * channels;
                    samples.clear();
                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                        samples.resize(count, 0.0);
                    } else {
                        let bytes = std::slice::from_raw_parts(data, count * sample_bytes);
                        format::read(self.format.sample, bytes, &mut samples);
                    }
                    if let Err(e) = capture.ReleaseBuffer(frames) {
                        return self.failed(&e);
                    }
                }
                shared.samples.on_input(&samples, self.format);
            }
        }
    }

    fn render(&self, render: &IAudioRenderClient, requests: &Receiver<Open>, shared: &Shared) -> Next {
        let exclusive = self.mode == StreamMode::Exclusive;
        let sample_bytes = format::bytes(self.format.sample);
        let channels = usize::from(self.format.channels);
        // Exclusive: every event wants one whole period. Shared: 1.5 periods queued in Windows,
        // the whole buffer on a 10 ms period or longer (a device may take two periods at once).
        let target = if exclusive || self.period >= self.format.rate / 100 {
            self.buffer
        } else {
            (self.period * 3 / 2).min(self.buffer)
        };
        let mut samples = Vec::new();
        // The output reopens itself at a quiet moment every 5 minutes or more (FR-003a).
        let mut resync = audio::resync::Resync::new(self.format.rate);
        loop {
            self.event.wait();
            if let Some(next) = control(requests, shared) {
                return next;
            }
            let padding = if exclusive {
                0
            } else {
                // SAFETY: a COM call on this side's thread.
                match unsafe { self.client.GetCurrentPadding() } {
                    Ok(frames) => frames,
                    Err(e) => return self.failed(&e),
                }
            };
            let frames = target.saturating_sub(padding);
            if frames == 0 {
                continue;
            }
            let count = frames as usize * channels;
            samples.clear();
            samples.resize(count, 0.0);
            shared.samples.fill(&mut samples, self.format);
            // SAFETY: COM calls on this side's thread; WASAPI's buffer holds `frames` frames of
            // the opened format until it is released.
            unsafe {
                let data = match render.GetBuffer(frames) {
                    Ok(data) => data,
                    Err(e) => return self.failed(&e),
                };
                format::write(
                    self.format.sample,
                    &samples,
                    std::slice::from_raw_parts_mut(data, count * sample_bytes),
                );
                if let Err(e) = render.ReleaseBuffer(frames, 0) {
                    return self.failed(&e);
                }
            }
            if resync.feed(&samples, channels) {
                log::info!("audio Output: resync (quiet after 5 minutes)");
                return Next::Reopen(Open {
                    device: self.endpoint.clone(),
                    exclusive: self.exclusive,
                });
            }
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: a COM call on this side's thread; the client is released right after.
        let _ = unsafe { self.client.Stop() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_map_as_research_r7() {
        let e = |code: HRESULT| windows::core::Error::from(code);
        assert_eq!(
            error(Side::Input, false, &e(AUDCLNT_E_DEVICE_INVALIDATED)),
            AudioError::Gone
        );
        assert_eq!(error(Side::Output, false, &e(NOT_FOUND)), AudioError::Gone);
        assert_eq!(error(Side::Input, true, &e(AUDCLNT_E_DEVICE_IN_USE)), AudioError::InUse);
        assert_eq!(
            error(Side::Output, true, &e(AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED)),
            AudioError::InUse
        );
        assert_eq!(
            error(Side::Output, true, &e(AUDCLNT_E_UNSUPPORTED_FORMAT)),
            AudioError::Format
        );
        assert_eq!(error(Side::Input, false, &e(E_ACCESSDENIED)), AudioError::Blocked);
        // A shared open refused because another app holds the device alone is in use too.
        assert_eq!(
            error(Side::Input, false, &e(AUDCLNT_E_DEVICE_IN_USE)),
            AudioError::InUse
        );
        // A shared open never reports format: that is a failure there.
        assert!(matches!(
            error(Side::Output, false, &e(AUDCLNT_E_UNSUPPORTED_FORMAT)),
            AudioError::Failed(_)
        ));
        assert!(matches!(
            error(Side::Output, false, &e(E_ACCESSDENIED)),
            AudioError::Failed(_)
        ));
    }
}
