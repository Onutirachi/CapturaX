//! Motor de áudio nativo (WASAPI).
//!
//! - **Fontes**: cada dispositivo de entrada ou app capturado roda numa thread própria e
//!   distribui as amostras para quem assinou (preview e/ou mixer), sem abrir o dispositivo duas vezes.
//! - **Preview**: entrada escolhida → saída escolhida, com buffer mínimo (o objetivo é jogar).
//! - **Mixer**: só roda durante gravação/transmissão; soma dispositivo, microfone e apps (menos
//!   o CapturaX e os excluídos) em blocos de 10 ms com timestamp no relógio QPC.

mod apps;
mod ring;
mod wasapi;

pub use apps::{display_name, AppEntry};
pub use wasapi::{ComGuard, DeviceInfo};

use ring::Ring;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wasapi::{boost_thread_priority, CaptureStream, RenderStream};
use windows::Win32::Media::Audio::{eCapture, eRender};

use crate::clock;

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
/// Bloco do mixer: 10 ms.
pub const BLOCK_FRAMES: usize = 480;

const fn ms_to_frames(ms: u32) -> usize {
    (SAMPLE_RATE as usize * ms as usize) / 1000
}

/// Volume atualizável em tempo real (f32 guardado em bits num atômico).
#[derive(Default)]
pub struct Gain {
    bits: AtomicU32,
    muted: AtomicBool,
}

impl Gain {
    pub fn new(volume: f32) -> Arc<Self> {
        let gain = Arc::new(Self::default());
        gain.set(volume, false);
        gain
    }

    pub fn set(&self, volume: f32, muted: bool) {
        self.bits.store(volume.clamp(0.0, 4.0).to_bits(), Ordering::Relaxed);
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        if self.muted.load(Ordering::Relaxed) {
            0.0
        } else {
            f32::from_bits(self.bits.load(Ordering::Relaxed))
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum SourceKey {
    Device(String),
    App(u32),
}

type Subscribers = Arc<Mutex<Vec<(u64, Arc<Ring>)>>>;

struct Source {
    stop: Arc<AtomicBool>,
    subscribers: Subscribers,
    thread: Option<JoinHandle<()>>,
}

struct Subscription {
    key: SourceKey,
    id: u64,
    ring: Arc<Ring>,
}

/// Bloco de áudio mixado (f32 estéreo 48 kHz) com o instante de captura no relógio QPC.
pub struct PcmBlock {
    pub pts_us: i64,
    pub samples: Arc<Vec<f32>>,
}

/// Recebe os blocos do mixer; devolve `false` para ser removido (sessão encerrada).
pub type PcmSink = Box<dyn FnMut(&PcmBlock) -> bool + Send>;

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewSettings {
    pub input: Option<String>,
    pub output: Option<String>,
    pub delay_ms: u32,
}

#[derive(Clone, Debug)]
pub struct MixSettings {
    /// (dispositivo, volume)
    pub inputs: Vec<(String, f32)>,
    /// Volume do áudio dos apps; `None` = sem áudio do sistema.
    pub system: Option<f32>,
    pub excluded: Vec<String>,
    pub offset_ms: i32,
}

struct Preview {
    settings: PreviewSettings,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    subscription: Option<Subscription>,
}

struct MixInput {
    key: SourceKey,
    ring: Arc<Ring>,
    gain: Arc<Gain>,
    primed: bool,
}

struct Mixer {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    inputs: Arc<Mutex<Vec<MixInput>>>,
    subscriptions: Arc<Mutex<Vec<Subscription>>>,
    outputs: Arc<Mutex<Vec<(u64, PcmSink)>>>,
    system_gain: Arc<Gain>,
    excluded: Arc<RwLock<Vec<String>>>,
    offset_us: Arc<AtomicI32>,
}

#[derive(Default)]
struct EngineState {
    sources: HashMap<SourceKey, Source>,
    next_id: u64,
    preview: Option<Preview>,
    mixer: Option<Mixer>,
}

#[derive(Clone)]
pub struct AudioEngine {
    state: Arc<Mutex<EngineState>>,
    preview_gain: Arc<Gain>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self { state: Arc::default(), preview_gain: Gain::new(1.0) }
    }

    pub fn list_inputs(&self) -> Result<Vec<DeviceInfo>, String> {
        let _com = ComGuard::new();
        wasapi::list_devices(eCapture).map_err(|e| e.to_string())
    }

    pub fn list_outputs(&self) -> Result<Vec<DeviceInfo>, String> {
        let _com = ComGuard::new();
        wasapi::list_devices(eRender).map_err(|e| e.to_string())
    }

    pub fn running_apps(&self) -> Result<Vec<AppEntry>, String> {
        let _com = ComGuard::new();
        apps::running_apps().map_err(|e| e.to_string())
    }

    // ---- Fontes -------------------------------------------------------------------------

    fn subscribe(&self, state: &mut EngineState, key: SourceKey, max_frames: usize) -> Subscription {
        state.next_id += 1;
        let id = state.next_id;
        let ring = Arc::new(Ring::new(max_frames));
        let source = state.sources.entry(key.clone()).or_insert_with(|| start_source(key.clone()));
        source.subscribers.lock().unwrap().push((id, ring.clone()));
        Subscription { key, id, ring }
    }

    /// Remove a assinatura; a fonte para quando ninguém mais escuta. Devolve a thread para
    /// o chamador esperar fora do lock.
    fn unsubscribe(state: &mut EngineState, subscription: &Subscription) -> Option<JoinHandle<()>> {
        let source = state.sources.get(&subscription.key)?;
        let empty = {
            let mut subscribers = source.subscribers.lock().unwrap();
            subscribers.retain(|(id, _)| *id != subscription.id);
            subscribers.is_empty()
        };
        if !empty {
            return None;
        }
        let mut source = state.sources.remove(&subscription.key)?;
        source.stop.store(true, Ordering::Relaxed);
        source.thread.take()
    }

    // ---- Preview ------------------------------------------------------------------------

    pub fn set_preview_volume(&self, volume: f32, muted: bool) {
        self.preview_gain.set(volume, muted);
    }

    /// Liga, troca ou desliga (`None`) o preview de áudio.
    pub fn set_preview(&self, settings: Option<PreviewSettings>) {
        let mut joins = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            if state.preview.as_ref().map(|p| &p.settings) == settings.as_ref() {
                return;
            }
            if let Some(mut preview) = state.preview.take() {
                preview.stop.store(true, Ordering::Relaxed);
                joins.extend(preview.thread.take());
                if let Some(subscription) = preview.subscription.take() {
                    joins.extend(Self::unsubscribe(&mut state, &subscription));
                }
            }
            if let Some(settings) = settings {
                let subscription = settings
                    .input
                    .clone()
                    .map(|input| self.subscribe(&mut state, SourceKey::Device(input), ms_to_frames(500)));
                let stop = Arc::new(AtomicBool::new(false));
                let thread = subscription.as_ref().map(|s| {
                    let (ring, stop, gain, settings) = (s.ring.clone(), stop.clone(), self.preview_gain.clone(), settings.clone());
                    spawn("capturax-audio-preview", move || run_preview(settings, ring, gain, stop))
                });
                state.preview = Some(Preview { settings, stop, thread, subscription });
            }
        }
        for join in joins {
            let _ = join.join();
        }
    }

    // ---- Mixer --------------------------------------------------------------------------

    /// Adiciona um consumidor do áudio mixado; liga o mixer se for o primeiro.
    pub fn add_mix_output(&self, settings: &MixSettings, sink: PcmSink) -> u64 {
        let mut state = self.state.lock().unwrap();
        state.next_id += 1;
        let id = state.next_id;
        if state.mixer.is_none() {
            let mixer = self.start_mixer(&mut state, settings);
            state.mixer = Some(mixer);
        }
        state.mixer.as_ref().unwrap().outputs.lock().unwrap().push((id, sink));
        id
    }

    pub fn remove_mix_output(&self, id: u64) {
        let mixer = {
            let mut state = self.state.lock().unwrap();
            let Some(mixer) = state.mixer.as_ref() else { return };
            let empty = {
                let mut outputs = mixer.outputs.lock().unwrap();
                outputs.retain(|(output, _)| *output != id);
                outputs.is_empty()
            };
            if !empty {
                return;
            }
            state.mixer.take()
        };
        if let Some(mixer) = mixer {
            self.stop_mixer(mixer);
        }
    }

    /// Volumes, exclusões e sincronia valem na hora; dispositivos só na próxima gravação.
    pub fn update_mix(&self, settings: &MixSettings) {
        let state = self.state.lock().unwrap();
        let Some(mixer) = state.mixer.as_ref() else { return };
        mixer.system_gain.set(settings.system.unwrap_or(0.0), false);
        *mixer.excluded.write().unwrap() = settings.excluded.clone();
        mixer.offset_us.store(settings.offset_ms * 1000, Ordering::Relaxed);
        for input in mixer.inputs.lock().unwrap().iter() {
            if let SourceKey::Device(device) = &input.key {
                if let Some((_, volume)) = settings.inputs.iter().find(|(id, _)| id == device) {
                    input.gain.set(*volume, false);
                }
            }
        }
    }

    fn start_mixer(&self, state: &mut EngineState, settings: &MixSettings) -> Mixer {
        let stop = Arc::new(AtomicBool::new(false));
        let inputs: Arc<Mutex<Vec<MixInput>>> = Arc::default();
        let subscriptions: Arc<Mutex<Vec<Subscription>>> = Arc::default();
        let system_gain = Gain::new(settings.system.unwrap_or(0.0));
        let excluded = Arc::new(RwLock::new(settings.excluded.clone()));
        let offset_us = Arc::new(AtomicI32::new(settings.offset_ms * 1000));

        let mut seen = HashSet::new();
        for (device, volume) in &settings.inputs {
            // O mesmo dispositivo duas vezes (placa = microfone?) entraria dobrado.
            if !seen.insert(device.clone()) {
                continue;
            }
            let subscription = self.subscribe(state, SourceKey::Device(device.clone()), MIX_RING_FRAMES);
            inputs.lock().unwrap().push(MixInput { key: subscription.key.clone(), ring: subscription.ring.clone(), gain: Gain::new(*volume), primed: false });
            subscriptions.lock().unwrap().push(subscription);
        }

        let outputs: Arc<Mutex<Vec<(u64, PcmSink)>>> = Arc::default();
        let mut threads = vec![{
            let (stop, inputs, outputs, offset_us) = (stop.clone(), inputs.clone(), outputs.clone(), offset_us.clone());
            spawn("capturax-audio-mixer", move || run_mixer(stop, inputs, outputs, offset_us))
        }];
        if settings.system.is_some() {
            let engine = self.clone();
            let (stop, inputs, subscriptions, system_gain, excluded) =
                (stop.clone(), inputs.clone(), subscriptions.clone(), system_gain.clone(), excluded.clone());
            threads.push(spawn("capturax-audio-apps", move || {
                engine.run_app_scanner(stop, inputs, subscriptions, system_gain, excluded)
            }));
        }
        Mixer { stop, threads, inputs, subscriptions, outputs, system_gain, excluded, offset_us }
    }

    fn stop_mixer(&self, mixer: Mixer) {
        mixer.stop.store(true, Ordering::Relaxed);
        for thread in mixer.threads {
            let _ = thread.join();
        }
        let mut joins = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for subscription in mixer.subscriptions.lock().unwrap().drain(..) {
                joins.extend(Self::unsubscribe(&mut state, &subscription));
            }
        }
        for join in joins {
            let _ = join.join();
        }
    }

    /// A cada 1,5 s confere quais apps tocam áudio e ajusta as capturas (entra app novo, sai
    /// app fechado ou excluído).
    fn run_app_scanner(
        &self,
        stop: Arc<AtomicBool>,
        inputs: Arc<Mutex<Vec<MixInput>>>,
        subscriptions: Arc<Mutex<Vec<Subscription>>>,
        system_gain: Arc<Gain>,
        excluded: Arc<RwLock<Vec<String>>>,
    ) {
        let _com = ComGuard::new();
        let mut last_scan: Option<Instant> = None;
        while !stop.load(Ordering::Relaxed) {
            if last_scan.is_some_and(|t| t.elapsed() < Duration::from_millis(1500)) {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            last_scan = Some(Instant::now());

            let wanted = match (apps::list_sessions(), apps::ProcessTable::snapshot()) {
                (Ok(sessions), Ok(table)) => apps::plan_captures(&sessions, &table, &excluded.read().unwrap()),
                (Err(error), _) | (_, Err(error)) => {
                    eprintln!("App audio scan failed: {error}");
                    continue;
                }
            };

            let current: HashSet<u32> = inputs
                .lock()
                .unwrap()
                .iter()
                .filter_map(|input| match input.key {
                    SourceKey::App(pid) => Some(pid),
                    _ => None,
                })
                .collect();

            let mut joins = Vec::new();
            {
                let mut state = self.state.lock().unwrap();
                for &pid in wanted.difference(&current) {
                    let subscription = self.subscribe(&mut state, SourceKey::App(pid), MIX_RING_FRAMES);
                    inputs.lock().unwrap().push(MixInput {
                        key: subscription.key.clone(),
                        ring: subscription.ring.clone(),
                        gain: system_gain.clone(),
                        primed: false,
                    });
                    subscriptions.lock().unwrap().push(subscription);
                }
                let removed: HashSet<u32> = current.difference(&wanted).copied().collect();
                if !removed.is_empty() {
                    let is_removed = |key: &SourceKey| matches!(key, SourceKey::App(pid) if removed.contains(pid));
                    inputs.lock().unwrap().retain(|input| !is_removed(&input.key));
                    let mut subscriptions = subscriptions.lock().unwrap();
                    for subscription in subscriptions.iter().filter(|s| is_removed(&s.key)) {
                        joins.extend(Self::unsubscribe(&mut state, subscription));
                    }
                    subscriptions.retain(|s| !is_removed(&s.key));
                }
            }
            for join in joins {
                let _ = join.join();
            }
        }
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    std::thread::Builder::new().name(name.into()).spawn(f).expect("spawn audio thread")
}

fn start_source(key: SourceKey) -> Source {
    let stop = Arc::new(AtomicBool::new(false));
    let subscribers: Subscribers = Arc::default();
    let thread = {
        let (key, stop, subscribers) = (key.clone(), stop.clone(), subscribers.clone());
        spawn("capturax-audio-source", move || run_source(key, stop, subscribers))
    };
    Source { stop, subscribers, thread: Some(thread) }
}

fn run_source(key: SourceKey, stop: Arc<AtomicBool>, subscribers: Subscribers) {
    let _com = ComGuard::new();
    boost_thread_priority();
    let mut reported = false;
    while !stop.load(Ordering::Relaxed) {
        let opened = match &key {
            SourceKey::Device(id) => CaptureStream::open_device(id),
            SourceKey::App(pid) => CaptureStream::open_process(*pid),
        };
        match opened {
            Ok(mut stream) => {
                reported = false;
                while !stop.load(Ordering::Relaxed) {
                    let result = stream.read(100, &mut |samples| {
                        for (_, ring) in subscribers.lock().unwrap().iter() {
                            ring.push(samples);
                        }
                    });
                    if let Err(error) = result {
                        eprintln!("Audio capture {key:?} stopped: {error}");
                        break;
                    }
                }
            }
            Err(error) => {
                if !reported {
                    eprintln!("Could not open audio {key:?}: {error}");
                    reported = true;
                }
            }
        }
        // App que fechou não volta com o mesmo PID; o scanner cuida disso.
        if matches!(key, SourceKey::App(_)) {
            break;
        }
        // Dispositivo desconectado: tenta de novo até voltar.
        for _ in 0..10 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Atraso alvo do Windows (fila de saída) no preview.
const PREVIEW_QUEUE_FRAMES: u32 = ms_to_frames(15) as u32;

fn run_preview(settings: PreviewSettings, ring: Arc<Ring>, gain: Arc<Gain>, stop: Arc<AtomicBool>) {
    let _com = ComGuard::new();
    boost_thread_priority();
    let delay_frames = ms_to_frames(settings.delay_ms);
    let mut reported = false;

    while !stop.load(Ordering::Relaxed) {
        let output = match RenderStream::open(settings.output.as_deref()) {
            Ok(output) => output,
            Err(error) => {
                if !reported {
                    eprintln!("Could not open audio output: {error}");
                    reported = true;
                }
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        reported = false;
        let target = PREVIEW_QUEUE_FRAMES.min(output.buffer_frames());
        // Só começa a tocar com o atraso pedido acumulado; volta a acumular depois de faltar áudio.
        let mut primed = false;

        while !stop.load(Ordering::Relaxed) {
            output.wait(100);
            let padding = match output.padding() {
                Ok(padding) => padding,
                Err(error) => {
                    eprintln!("Audio output stopped: {error}");
                    break;
                }
            };
            let frames = target.saturating_sub(padding);
            if frames == 0 {
                continue;
            }

            // Se a captura andar mais rápido que a saída, o atraso cresceria aos poucos: corta.
            let level = ring.len_frames();
            if level > delay_frames + ms_to_frames(40) {
                ring.trim_to(delay_frames + ms_to_frames(10));
            }
            if !primed && ring.len_frames() >= delay_frames + frames as usize {
                primed = true;
            }

            let volume = gain.get();
            let result = output.write(frames, &mut |buffer| {
                let read = if primed { ring.read(buffer) } else { 0 };
                for sample in &mut buffer[..read * CHANNELS] {
                    *sample *= volume;
                }
                buffer[read * CHANNELS..].fill(0.0);
                if primed && read < frames as usize {
                    primed = false;
                }
            });
            if let Err(error) = result {
                eprintln!("Audio output stopped: {error}");
                break;
            }
        }
    }
}

/// Folga de cada entrada do mixer antes de começar a consumir (absorve o tamanho dos pacotes).
const MIX_JITTER_FRAMES: usize = ms_to_frames(30);
/// Acima disso o atraso está crescendo (relógio do dispositivo adiantado): corta de volta.
const MIX_MAX_FRAMES: usize = ms_to_frames(120);
const MIX_RING_FRAMES: usize = ms_to_frames(1000);
/// Atraso médio entre o som acontecer e sair do mixer (folga + pacote).
const MIX_LATENCY_US: i64 = 35_000;

fn run_mixer(stop: Arc<AtomicBool>, inputs: Arc<Mutex<Vec<MixInput>>>, outputs: Arc<Mutex<Vec<(u64, PcmSink)>>>, offset_us: Arc<AtomicI32>) {
    boost_thread_priority();
    let start = clock::now_us();
    let mut emitted: i64 = 0;
    let mut scratch = vec![0.0f32; BLOCK_FRAMES * CHANNELS];

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(4));
        let due = (clock::now_us() - start) * SAMPLE_RATE as i64 / 1_000_000;
        // Ficou muito para trás (ex.: PC suspenso): pula em vez de despejar segundos de uma vez.
        if due - emitted > SAMPLE_RATE as i64 {
            emitted = due - due % BLOCK_FRAMES as i64;
        }
        while emitted + BLOCK_FRAMES as i64 <= due {
            let mut mix = vec![0.0f32; BLOCK_FRAMES * CHANNELS];
            for input in inputs.lock().unwrap().iter_mut() {
                let level = input.ring.len_frames();
                if level > MIX_MAX_FRAMES {
                    input.ring.trim_to(MIX_JITTER_FRAMES);
                }
                if !input.primed {
                    input.primed = level >= MIX_JITTER_FRAMES;
                    if !input.primed {
                        continue;
                    }
                }
                let read = input.ring.read(&mut scratch);
                if read < BLOCK_FRAMES {
                    input.primed = false;
                }
                let volume = input.gain.get();
                for (out, sample) in mix.iter_mut().zip(&scratch[..read * CHANNELS]) {
                    *out += sample * volume;
                }
            }
            for sample in &mut mix {
                *sample = sample.clamp(-1.0, 1.0);
            }
            let pts_us = start + emitted * 1_000_000 / SAMPLE_RATE as i64 - MIX_LATENCY_US + offset_us.load(Ordering::Relaxed) as i64;
            let block = PcmBlock { pts_us, samples: Arc::new(mix) };
            outputs.lock().unwrap().retain_mut(|(_, sink)| sink(&block));
            emitted += BLOCK_FRAMES as i64;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Testes contra o WASAPI de verdade desta máquina: `cargo test --lib audio -- --ignored --nocapture`
    use super::*;

    #[test]
    #[ignore]
    fn audio_enumerate_only() {
        let engine = AudioEngine::new();
        for _ in 0..20 {
            engine.list_inputs().unwrap();
            engine.list_outputs().unwrap();
            engine.running_apps().unwrap();
        }
        let v: Vec<Vec<u8>> = (0..10000).map(|i| vec![0u8; i % 4096]).collect();
        println!("enumerate ok {}", v.len());
    }

    #[test]
    #[ignore]
    fn audio_loopback_only() {
        let _com = ComGuard::new();
        assert!(CaptureStream::open_process(std::process::id()).is_ok());
    }

    #[test]
    #[ignore]
    fn audio_apps_levels() {
        let _com = ComGuard::new();
        let sessions = apps::list_sessions().unwrap();
        let table = apps::ProcessTable::snapshot().unwrap();
        let plan = apps::plan_captures(&sessions, &table, &["discord.exe".to_string()]);
        for s in &sessions {
            println!("session pid={} exe={:?} active={} planned={}", s.pid, table.exe(s.pid), s.active, plan.contains(&s.pid));
        }
        for &pid in &plan {
            let mut stream = match CaptureStream::open_process(pid) {
                Ok(stream) => stream,
                Err(error) => {
                    println!("pid {pid}: open failed {error}");
                    continue;
                }
            };
            let (mut count, mut max) = (0usize, 0f32);
            let start = Instant::now();
            while start.elapsed() < Duration::from_millis(1500) {
                stream.read(100, &mut |samples| {
                    count += samples.len();
                    max = samples.iter().fold(max, |m, s| m.max(s.abs()));
                }).unwrap();
            }
            println!("pid {pid} {:?}: {} frames in 1.5 s, peak {max:.3}", table.exe(pid), count / 2);
        }
    }

    #[test]
    #[ignore]
    fn audio_loopback_hears_tone() {
        let stop = Arc::new(AtomicBool::new(false));
        let player = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let _com = ComGuard::new();
                let output = RenderStream::open(None).unwrap();
                let mut phase = 0f32;
                while !stop.load(Ordering::Relaxed) {
                    output.wait(100);
                    let frames = (output.buffer_frames() / 2).saturating_sub(output.padding().unwrap());
                    output.write(frames, &mut |buffer| {
                        for frame in buffer.chunks_mut(2) {
                            let v = (phase * std::f32::consts::TAU).sin() * 0.01;
                            phase = (phase + 440.0 / SAMPLE_RATE as f32).fract();
                            frame.fill(v);
                        }
                    }).unwrap();
                }
            })
        };
        let _com = ComGuard::new();
        let mut stream = CaptureStream::open_process(std::process::id()).unwrap();
        let (mut count, mut max) = (0usize, 0f32);
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(1500) {
            stream.read(100, &mut |samples| {
                count += samples.len();
                max = samples.iter().fold(max, |m, s| m.max(s.abs()));
            }).unwrap();
        }
        stop.store(true, Ordering::Relaxed);
        player.join().unwrap();
        println!("own tone: {} frames, peak {max:.4}", count / 2);
        assert!(max > 0.005, "loopback did not capture the tone");
    }

    #[test]
    #[ignore]
    fn audio_smoke() {
        let engine = AudioEngine::new();
        let inputs = engine.list_inputs().unwrap();
        let outputs = engine.list_outputs().unwrap();
        println!("inputs: {:?}", inputs.iter().map(|d| (&d.name, d.is_default)).collect::<Vec<_>>());
        println!("outputs: {:?}", outputs.iter().map(|d| (&d.name, d.is_default)).collect::<Vec<_>>());
        println!("apps: {:?}", engine.running_apps().unwrap());

        let _com = ComGuard::new();
        // Loopback do próprio processo de teste: valida a ativação assíncrona.
        let mut stream = CaptureStream::open_process(std::process::id()).expect("process loopback");
        let mut received = 0;
        for _ in 0..5 {
            stream.read(100, &mut |samples| received += samples.len()).unwrap();
        }
        println!("process loopback ok, {received} samples in 500 ms");

        let output = RenderStream::open(None).expect("default output");
        let target = output.buffer_frames() / 2;
        for _ in 0..10 {
            output.wait(100);
            let frames = target.saturating_sub(output.padding().unwrap());
            output.write(frames, &mut |buffer| buffer.fill(0.0)).unwrap();
        }
        println!("render ok (buffer {} frames)", output.buffer_frames());

        // Mixer com áudio do sistema por 1,5 s: precisa produzir ~150 blocos de 10 ms.
        let blocks = Arc::new(AtomicU32::new(0));
        let peak = Arc::new(AtomicU32::new(0));
        let counter = blocks.clone();
        let level = peak.clone();
        let settings = MixSettings { inputs: vec![], system: Some(1.0), excluded: vec!["discord.exe".into()], offset_ms: 0 };
        let id = engine.add_mix_output(&settings, Box::new(move |block| {
            counter.fetch_add(1, Ordering::Relaxed);
            let max = block.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
            level.fetch_max((max * 1000.0) as u32, Ordering::Relaxed);
            true
        }));
        std::thread::sleep(Duration::from_millis(4000));
        engine.remove_mix_output(id);
        println!("mixer produced {} blocks, peak {:.3}", blocks.load(Ordering::Relaxed), peak.load(Ordering::Relaxed) as f32 / 1000.0);
        assert!(blocks.load(Ordering::Relaxed) > 100);
    }
}
