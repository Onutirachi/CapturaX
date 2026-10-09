//! Camada fina sobre o WASAPI: dispositivos, captura (dispositivo ou processo) e saída.
//! Tudo trabalha em f32 estéreo 48 kHz; o Windows converte o formato do dispositivo
//! (`AUTOCONVERTPCM`), e o que não der para converter lá é reamostrado aqui.

use serde::Serialize;
use std::mem::ManuallyDrop;
use std::sync::mpsc;
use std::time::Duration;
use windows::core::{implement, Interface, Ref, Result, HRESULT, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::{PKEY_Device_ContainerId, PKEY_Device_FriendlyName};
use windows::Win32::Foundation::{CloseHandle, E_FAIL, HANDLE, RPC_E_CHANGED_MODE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, SPEAKER_FRONT_LEFT, SPEAKER_FRONT_RIGHT, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::StructuredStorage::{PropVariantToGUID, PropVariantToStringAlloc, PROPVARIANT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, BLOB, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
use windows::Win32::System::Threading::{AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject};
use windows::Win32::System::Variant::VT_BLOB;

use super::{CHANNELS, SAMPLE_RATE};

/// Inicializa o COM (MTA) na thread atual e desfaz ao sair do escopo.
pub struct ComGuard {
    initialized: bool,
}

impl ComGuard {
    pub fn new() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        // RPC_E_CHANGED_MODE: a thread já é STA; dá para usar o COM, mas não é nossa para desfazer.
        Self { initialized: hr.is_ok() && hr != RPC_E_CHANGED_MODE }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

/// Prioridade de áudio do Windows (MMCSS) para a thread atual; diminui estalos sob carga.
pub fn boost_thread_priority() {
    let mut task_index = 0u32;
    let _ = unsafe { AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut task_index) };
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    /// Aparelho físico (o vídeo e o áudio de uma placa USB têm o mesmo).
    pub container_id: Option<String>,
}

fn enumerator() -> Result<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn device_id(device: &IMMDevice) -> Result<String> {
    unsafe {
        let id = device.GetId()?;
        let text = id.to_string().unwrap_or_default();
        CoTaskMemFree(Some(id.0 as _));
        Ok(text)
    }
}

fn device_name(device: &IMMDevice) -> Result<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ)?;
        // O PROPVARIANT se limpa sozinho no Drop.
        let value = store.GetValue(&PKEY_Device_FriendlyName)?;
        let text = PropVariantToStringAlloc(&value)?;
        let name = text.to_string().unwrap_or_default();
        CoTaskMemFree(Some(text.0 as _));
        Ok(name)
    }
}

fn device_container(device: &IMMDevice) -> Result<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ)?;
        let value = store.GetValue(&PKEY_Device_ContainerId)?;
        Ok(format!("{:?}", PropVariantToGUID(&value)?).to_lowercase())
    }
}

pub fn list_devices(flow: EDataFlow) -> Result<Vec<DeviceInfo>> {
    let enumerator = enumerator()?;
    let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(flow, eConsole) }.and_then(|d| device_id(&d)).ok();
    let collection = unsafe { enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)? };
    let mut devices = Vec::new();
    for i in 0..unsafe { collection.GetCount()? } {
        let device = unsafe { collection.Item(i)? };
        let id = device_id(&device)?;
        let name = device_name(&device).unwrap_or_else(|_| id.clone());
        let container_id = device_container(&device).ok();
        devices.push(DeviceInfo { is_default: default_id.as_deref() == Some(id.as_str()), id, name, container_id });
    }
    Ok(devices)
}

/// Todos os endpoints de saída ativos (para achar as sessões de áudio dos apps).
pub fn render_devices() -> Result<Vec<IMMDevice>> {
    let enumerator = enumerator()?;
    let collection = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)? };
    (0..unsafe { collection.GetCount()? }).map(|i| unsafe { collection.Item(i) }).collect()
}

fn get_device(id: Option<&str>, flow: EDataFlow) -> Result<IMMDevice> {
    let enumerator = enumerator()?;
    match id {
        Some(id) => {
            let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { enumerator.GetDevice(PCWSTR(wide.as_ptr())) }
        }
        None => unsafe { enumerator.GetDefaultAudioEndpoint(flow, eConsole) },
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SampleKind {
    F32,
    I16,
}

#[derive(Clone, Copy)]
struct Format {
    kind: SampleKind,
    rate: u32,
    /// `WAVEFORMATEXTENSIBLE` completo ou só o `WAVEFORMATEX` simples (o loopback por processo
    /// é mais exigente; o exemplo oficial usa o formato simples).
    extensible: bool,
}

impl Format {
    fn wave_format(&self) -> WAVEFORMATEXTENSIBLE {
        let bits: u16 = match self.kind {
            SampleKind::F32 => 32,
            SampleKind::I16 => 16,
        };
        let block_align = CHANNELS as u16 * bits / 8;
        let tag = match (self.extensible, self.kind) {
            (true, _) => WAVE_FORMAT_EXTENSIBLE as u16,
            (false, SampleKind::F32) => 3, // WAVE_FORMAT_IEEE_FLOAT
            (false, SampleKind::I16) => WAVE_FORMAT_PCM as u16,
        };
        WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: tag,
                nChannels: CHANNELS as u16,
                nSamplesPerSec: self.rate,
                nAvgBytesPerSec: self.rate * block_align as u32,
                nBlockAlign: block_align,
                wBitsPerSample: bits,
                cbSize: if self.extensible { 22 } else { 0 },
            },
            Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: bits },
            dwChannelMask: SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT,
            SubFormat: match self.kind {
                SampleKind::F32 => KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
                SampleKind::I16 => KSDATAFORMAT_SUBTYPE_PCM,
            },
        }
    }
}

const DEVICE_FORMATS: &[Format] = &[Format { kind: SampleKind::F32, rate: SAMPLE_RATE, extensible: true }];
const PROCESS_FORMATS: &[Format] = &[
    Format { kind: SampleKind::F32, rate: SAMPLE_RATE, extensible: true },
    Format { kind: SampleKind::F32, rate: SAMPLE_RATE, extensible: false },
    Format { kind: SampleKind::I16, rate: SAMPLE_RATE, extensible: false },
    Format { kind: SampleKind::I16, rate: 44_100, extensible: false },
];

/// 20 ms de buffer no WASAPI: folga suficiente sem somar atraso perceptível.
const BUFFER_HNS: i64 = 200_000;

fn initialize(client: &IAudioClient, flags: u32, formats: &[Format]) -> Result<Format> {
    let mut last_error = windows::core::Error::from(E_FAIL);
    for format in formats {
        let wave = format.wave_format();
        let convert = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        for extra in [convert, 0] {
            match unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags | extra, BUFFER_HNS, 0, &wave.Format, None) } {
                Ok(()) => return Ok(*format),
                Err(error) => last_error = error,
            }
        }
    }
    Err(last_error)
}

struct Event(HANDLE);

impl Event {
    fn new() -> Result<Self> {
        Ok(Self(unsafe { CreateEventW(None, false, false, None)? }))
    }

    /// `true` se o evento disparou, `false` no timeout.
    fn wait(&self, timeout_ms: u32) -> bool {
        unsafe { WaitForSingleObject(self.0, timeout_ms) == WAIT_OBJECT_0 }
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

// SAFETY: um HANDLE de evento pode ser usado de qualquer thread.
unsafe impl Send for Event {}

/// Reamostragem linear (só para o caso raro de o loopback por processo não aceitar 48 kHz).
struct Resampler {
    step: f64,
    position: f64,
    previous: [f32; CHANNELS],
}

impl Resampler {
    fn new(from_rate: u32) -> Self {
        Self { step: from_rate as f64 / SAMPLE_RATE as f64, position: 0.0, previous: [0.0; CHANNELS] }
    }

    fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        let frames = input.len() / CHANNELS;
        let frame = |i: isize| -> [f32; CHANNELS] {
            if i < 0 {
                self.previous
            } else {
                let i = i as usize * CHANNELS;
                [input[i], input[i + 1]]
            }
        };
        // `position` é relativo ao primeiro frame deste bloco; -1 = último frame do bloco anterior.
        while self.position < frames as f64 - 1.0 {
            let base = self.position.floor();
            let t = (self.position - base) as f32;
            let (a, b) = (frame(base as isize), frame(base as isize + 1));
            for c in 0..CHANNELS {
                output.push(a[c] + (b[c] - a[c]) * t);
            }
            self.position += self.step;
        }
        if frames > 0 {
            self.previous = frame(frames as isize - 1);
            self.position -= frames as f64;
        }
    }
}

/// Fluxo de captura: dispositivo de entrada ou áudio de uma árvore de processos.
pub struct CaptureStream {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: Event,
    format: Format,
    resampler: Option<Resampler>,
    scratch: Vec<f32>,
    resampled: Vec<f32>,
}

// SAFETY: o stream é criado e usado numa única thread MTA (a thread de captura dona dele).
unsafe impl Send for CaptureStream {}

impl CaptureStream {
    pub fn open_device(id: &str) -> Result<Self> {
        let device = get_device(Some(id), eCapture)?;
        let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
        Self::init(client, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_FORMATS)
    }

    /// Captura o áudio que a árvore de processos de `pid` toca (Windows 10 2004+).
    pub fn open_process(pid: u32) -> Result<Self> {
        let client = activate_process_loopback(pid)?;
        Self::init(client, AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK, PROCESS_FORMATS)
    }

    fn init(client: IAudioClient, flags: u32, formats: &[Format]) -> Result<Self> {
        let format = initialize(&client, flags, formats)?;
        let event = Event::new()?;
        unsafe { client.SetEventHandle(event.0)? };
        let capture: IAudioCaptureClient = unsafe { client.GetService()? };
        unsafe { client.Start()? };
        Ok(Self {
            client,
            capture,
            event,
            resampler: (format.rate != SAMPLE_RATE).then(|| Resampler::new(format.rate)),
            format,
            scratch: Vec::new(),
            resampled: Vec::new(),
        })
    }

    /// Espera até `timeout_ms` por dados e entrega tudo o que chegou como f32 estéreo 48 kHz.
    pub fn read(&mut self, timeout_ms: u32, sink: &mut dyn FnMut(&[f32])) -> Result<()> {
        self.event.wait(timeout_ms);
        loop {
            let frames_available = unsafe { self.capture.GetNextPacketSize()? };
            if frames_available == 0 {
                return Ok(());
            }
            let mut data = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            unsafe { self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)? };

            let samples = frames as usize * CHANNELS;
            self.scratch.clear();
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                self.scratch.resize(samples, 0.0);
            } else {
                match self.format.kind {
                    SampleKind::F32 => {
                        let slice = unsafe { std::slice::from_raw_parts(data as *const f32, samples) };
                        self.scratch.extend_from_slice(slice);
                    }
                    SampleKind::I16 => {
                        let slice = unsafe { std::slice::from_raw_parts(data as *const i16, samples) };
                        self.scratch.extend(slice.iter().map(|&s| s as f32 / 32768.0));
                    }
                }
            }
            unsafe { self.capture.ReleaseBuffer(frames)? };

            match &mut self.resampler {
                Some(resampler) => {
                    self.resampled.clear();
                    resampler.process(&self.scratch, &mut self.resampled);
                    sink(&self.resampled);
                }
                None => sink(&self.scratch),
            }
        }
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        let _ = unsafe { self.client.Stop() };
    }
}

/// Saída de áudio (preview) em f32 estéreo 48 kHz.
pub struct RenderStream {
    client: IAudioClient,
    render: IAudioRenderClient,
    event: Event,
    buffer_frames: u32,
}

unsafe impl Send for RenderStream {}

impl RenderStream {
    /// `id = None` = saída padrão do Windows.
    pub fn open(id: Option<&str>) -> Result<Self> {
        let device = get_device(id, eRender)?;
        let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
        initialize(&client, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_FORMATS)?;
        let event = Event::new()?;
        unsafe { client.SetEventHandle(event.0)? };
        let render: IAudioRenderClient = unsafe { client.GetService()? };
        let buffer_frames = unsafe { client.GetBufferSize()? };
        unsafe { client.Start()? };
        Ok(Self { client, render, event, buffer_frames })
    }

    pub fn wait(&self, timeout_ms: u32) -> bool {
        self.event.wait(timeout_ms)
    }

    /// Frames já na fila do Windows, ainda não tocados.
    pub fn padding(&self) -> Result<u32> {
        unsafe { self.client.GetCurrentPadding() }
    }

    pub fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }

    /// Escreve `frames` frames; `fill` recebe o buffer f32 intercalado para preencher.
    pub fn write(&self, frames: u32, fill: &mut dyn FnMut(&mut [f32])) -> Result<()> {
        if frames == 0 {
            return Ok(());
        }
        unsafe {
            let data = self.render.GetBuffer(frames)?;
            let buffer = std::slice::from_raw_parts_mut(data as *mut f32, frames as usize * CHANNELS);
            fill(buffer);
            self.render.ReleaseBuffer(frames, 0)
        }
    }
}

impl Drop for RenderStream {
    fn drop(&mut self) {
        let _ = unsafe { self.client.Stop() };
    }
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivationHandler {
    done: mpsc::SyncSender<()>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivationHandler_Impl {
    fn ActivateCompleted(&self, _operation: Ref<'_, IActivateAudioInterfaceAsyncOperation>) -> Result<()> {
        let _ = self.done.try_send(());
        Ok(())
    }
}

fn activate_process_loopback(pid: u32) -> Result<IAudioClient> {
    let (done, completed) = mpsc::sync_channel(1);
    let handler: IActivateAudioInterfaceCompletionHandler = ActivationHandler { done }.into();

    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    // O `Drop` do PROPVARIANT chama PropVariantClear, que tentaria liberar `params` (que está na
    // pilha, não no heap do COM): por isso o ManuallyDrop.
    let mut variant = ManuallyDrop::new(PROPVARIANT::default());
    unsafe {
        let inner = &mut *variant.Anonymous.Anonymous;
        inner.vt = VT_BLOB;
        inner.Anonymous.blob = BLOB {
            cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
            pBlobData: &mut params as *mut _ as *mut u8,
        };
    }
    let operation = unsafe { ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&*variant), &handler)? };
    completed.recv_timeout(Duration::from_secs(5)).map_err(|_| windows::core::Error::from(E_FAIL))?;

    let mut result = HRESULT(0);
    let mut interface = None;
    unsafe { operation.GetActivateResult(&mut result, &mut interface)? };
    result.ok()?;
    interface.ok_or_else(|| windows::core::Error::from(E_FAIL))?.cast()
}
