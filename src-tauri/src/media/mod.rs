//! Sessões de mídia: gravação (arquivo) e transmissão (HLS).
//!
//! O vídeo chega do WebView já codificado na GPU (WebCodecs), em pacotes H.264/HEVC com
//! timestamp no relógio QPC. O áudio vem do mixer nativo e é codificado em AAC aqui. Cada sessão
//! roda numa thread própria que junta os dois num MP4 fragmentado e entrega para o destino.

pub mod aac;
mod bitstream;
mod file;
pub mod fmp4;

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::audio::{AudioEngine, ComGuard, MixSettings, PcmBlock, CHANNELS, SAMPLE_RATE};
use crate::clock;
use aac::{AacEncoder, AUDIO_SPECIFIC_CONFIG};
use fmp4::{AudioTrack, Fragment, Fragmenter, Sample, VideoCodec, VideoTrack, VIDEO_TIMESCALE};

/// Destino dos fragmentos (arquivo ou servidor HLS).
pub trait FragmentSink: Send {
    /// `codecs`: lista RFC 6381 das faixas (`avc1.640033,mp4a.40.2`).
    fn init(&mut self, bytes: &[u8], moov_offset: usize, codecs: &str) -> std::io::Result<()>;
    fn fragment(&mut self, fragment: &Fragment) -> std::io::Result<()>;
    /// Devolve o arquivo final, se houver.
    fn finish(&mut self, video: &VideoTrack, audio: Option<&AudioTrack>) -> std::io::Result<Option<PathBuf>>;
}

pub use file::FileSink;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum SessionKind {
    Recording,
    Stream,
}

pub struct SessionOptions {
    pub kind: SessionKind,
    pub sink: Box<dyn FragmentSink>,
    pub audio: Option<MixSettings>,
    pub audio_bitrate_kbps: u32,
    /// Duração mínima de cada fragmento (corta sempre num keyframe).
    pub fragment_secs: f64,
    /// Baixa latência: corta também a cada `part_secs`, sem esperar o keyframe.
    pub part_secs: Option<f64>,
}

enum Message {
    VideoConfig(VideoTrack),
    Video { data: Vec<u8>, pts_us: i64, key: bool },
    Audio { samples: Arc<Vec<f32>>, pts_us: i64 },
    Stop(mpsc::Sender<Result<Option<PathBuf>, String>>),
}

#[derive(Default)]
struct Stats {
    started_us: AtomicI64,
    bytes: AtomicU64,
    fragments: AtomicU32,
}

struct Session {
    kind: SessionKind,
    sender: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
    mix_output: Option<u64>,
    stats: Arc<Stats>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub id: u32,
    pub kind: SessionKind,
    /// Segundos desde o primeiro keyframe (0 enquanto espera o vídeo começar).
    pub elapsed_secs: f64,
    pub bytes: u64,
}

#[derive(Clone)]
pub struct MediaManager {
    sessions: Arc<Mutex<HashMap<u32, Session>>>,
    next_id: Arc<AtomicU32>,
    audio: AudioEngine,
}

impl MediaManager {
    pub fn new(audio: AudioEngine) -> Self {
        Self { sessions: Arc::default(), next_id: Arc::new(AtomicU32::new(1)), audio }
    }

    pub fn start(&self, options: SessionOptions) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        let stats = Arc::new(Stats::default());

        let mix_output = options.audio.as_ref().map(|settings| {
            let sender = sender.clone();
            self.audio.add_mix_output(
                settings,
                Box::new(move |block: &PcmBlock| sender.send(Message::Audio { samples: block.samples.clone(), pts_us: block.pts_us }).is_ok()),
            )
        });

        let kind = options.kind;
        let thread = {
            let stats = stats.clone();
            std::thread::Builder::new()
                .name(format!("capturax-media-{id}"))
                .spawn(move || run_session(options, receiver, stats))
                .expect("spawn media session")
        };
        self.sessions.lock().unwrap().insert(id, Session { kind, sender, thread: Some(thread), mix_output, stats });
        id
    }

    pub fn video_config(&self, id: u32, track: VideoTrack) {
        if let Some(session) = self.sessions.lock().unwrap().get(&id) {
            let _ = session.sender.send(Message::VideoConfig(track));
        }
    }

    pub fn video_chunk(&self, id: u32, data: Vec<u8>, pts_us: i64, key: bool) {
        if let Some(session) = self.sessions.lock().unwrap().get(&id) {
            let _ = session.sender.send(Message::Video { data, pts_us, key });
        }
    }

    /// Encerra a sessão e espera o arquivo ser finalizado.
    pub fn stop(&self, id: u32) -> Result<Option<PathBuf>, String> {
        let Some(mut session) = self.sessions.lock().unwrap().remove(&id) else { return Ok(None) };
        if let Some(output) = session.mix_output {
            self.audio.remove_mix_output(output);
        }
        let (reply, result) = mpsc::channel();
        let _ = session.sender.send(Message::Stop(reply));
        let result = result.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| Err("Session did not finish".into()));
        if let Some(thread) = session.thread.take() {
            let _ = thread.join();
        }
        result
    }

    pub fn stop_all(&self) {
        let ids: Vec<u32> = self.sessions.lock().unwrap().keys().copied().collect();
        for id in ids {
            let _ = self.stop(id);
        }
    }

    pub fn status(&self) -> Vec<SessionStatus> {
        let now = clock::now_us();
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .map(|(&id, session)| {
                let started = session.stats.started_us.load(Ordering::Relaxed);
                SessionStatus {
                    id,
                    kind: session.kind,
                    elapsed_secs: if started == 0 { 0.0 } else { (now - started) as f64 / 1e6 },
                    bytes: session.stats.bytes.load(Ordering::Relaxed),
                }
            })
            .collect()
    }
}

/// Áudio guardado enquanto o primeiro keyframe não chega (o início da gravação é o vídeo).
const MAX_PENDING_AUDIO_US: i64 = 3_000_000;

struct SessionState {
    sink: Box<dyn FragmentSink>,
    fragmenter: Fragmenter,
    video: Option<VideoTrack>,
    audio_track: Option<AudioTrack>,
    aac: Option<AacEncoder>,
    base_us: Option<i64>,
    last_video_dts: Option<u64>,
    pending_audio: VecDeque<(Arc<Vec<f32>>, i64)>,
    /// Pacotes de vídeo em Annex B (decidido pela config: sem `description`).
    annexb: bool,
    audio_started: bool,
    aac_frames: u64,
    stats: Arc<Stats>,
    error: Option<String>,
}

impl SessionState {
    fn emit(&mut self, fragment: Option<Fragment>) {
        let Some(fragment) = fragment else { return };
        if self.error.is_some() {
            return;
        }
        match self.sink.fragment(&fragment) {
            Ok(()) => {
                self.stats.bytes.fetch_add(fragment.bytes.len() as u64, Ordering::Relaxed);
                self.stats.fragments.fetch_add(1, Ordering::Relaxed);
            }
            Err(error) => {
                eprintln!("Could not write media fragment: {error}");
                self.error = Some(error.to_string());
            }
        }
    }

    fn on_video(&mut self, data: Vec<u8>, pts_us: i64, key: bool) {
        let Some(video) = self.video.as_mut() else { return };
        // O encoder HEVC do Chromium manda Annex B sem `description`: os parâmetros vêm no
        // primeiro keyframe e o resto é convertido para o formato do MP4. Com `description`, os
        // pacotes já vêm no formato do MP4 (e não dá para detectar pelo conteúdo: um tamanho de
        // 256–511 bytes também começa com 00 00 01).
        let annexb = self.annexb;
        if video.description.is_empty() {
            if !(key && bitstream::is_annexb(&data)) {
                return;
            }
            match bitstream::build_description(video.codec, &data) {
                Some(description) => video.description = description,
                None => return,
            }
        }
        let data = if annexb { bitstream::to_length_prefixed(video.codec, &data) } else { data };
        let video = video.clone();
        let base = match self.base_us {
            Some(base) => base,
            None => {
                if !key {
                    return;
                }
                self.base_us = Some(pts_us);
                self.stats.started_us.store(clock::now_us(), Ordering::Relaxed);
                let (init, moov_offset) = fmp4::init_segment(&video, self.audio_track.as_ref());
                let codecs = match self.audio_track {
                    Some(_) => format!("{},mp4a.40.2", video.codec_string),
                    None => video.codec_string.clone(),
                };
                if let Err(error) = self.sink.init(&init, moov_offset, &codecs) {
                    eprintln!("Could not write media header: {error}");
                    self.error = Some(error.to_string());
                }
                for (samples, pts) in std::mem::take(&mut self.pending_audio) {
                    self.on_audio(&samples, pts);
                }
                pts_us
            }
        };
        let mut dts = ((pts_us - base).max(0) as i128 * VIDEO_TIMESCALE as i128 / 1_000_000) as u64;
        if let Some(last) = self.last_video_dts {
            if dts <= last {
                dts = last + 1;
            }
        }
        self.last_video_dts = Some(dts);
        let fragment = self.fragmenter.push_video(Sample { data, dts, sync: key });
        self.emit(fragment);
    }

    fn on_audio(&mut self, samples: &[f32], pts_us: i64) {
        if self.aac.is_none() {
            return;
        }
        let Some(base) = self.base_us else {
            self.pending_audio.push_back((Arc::new(samples.to_vec()), pts_us));
            while self.pending_audio.front().is_some_and(|(_, pts)| pts_us - pts > MAX_PENDING_AUDIO_US) {
                self.pending_audio.pop_front();
            }
            return;
        };

        let mut padded;
        let mut samples = samples;
        if !self.audio_started {
            // Alinha o começo do áudio exatamente com o primeiro keyframe.
            let offset_frames = (pts_us - base) * SAMPLE_RATE as i64 / 1_000_000;
            if offset_frames < 0 {
                let skip = (-offset_frames) as usize * CHANNELS;
                if skip >= samples.len() {
                    return;
                }
                samples = &samples[skip..];
            } else if offset_frames > 0 {
                padded = vec![0.0; (offset_frames as usize).min(SAMPLE_RATE as usize) * CHANNELS];
                padded.extend_from_slice(samples);
                samples = &padded;
            }
            self.audio_started = true;
        }

        let mut frames = Vec::new();
        if let Err(error) = self.aac.as_mut().unwrap().encode(samples, &mut frames) {
            eprintln!("AAC encoder failed: {error}");
            self.aac = None;
            return;
        }
        for data in frames {
            let dts = self.aac_frames * aac::FRAME_SAMPLES as u64;
            self.aac_frames += 1;
            self.fragmenter.push_audio(Sample { data, dts, sync: true });
        }
    }
}

fn run_session(options: SessionOptions, receiver: mpsc::Receiver<Message>, stats: Arc<Stats>) {
    let _com = ComGuard::new();
    let aac = match options.audio {
        Some(_) => match AacEncoder::new(options.audio_bitrate_kbps) {
            Ok(encoder) => Some(encoder),
            Err(error) => {
                eprintln!("Could not create AAC encoder, recording without audio: {error}");
                None
            }
        },
        None => None,
    };
    let audio_track = aac.as_ref().map(|encoder| AudioTrack {
        config: AUDIO_SPECIFIC_CONFIG.to_vec(),
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS as u16,
        bitrate: encoder.bitrate,
    });

    let mut state = SessionState {
        sink: options.sink,
        fragmenter: Fragmenter::new(options.fragment_secs, options.part_secs, SAMPLE_RATE),
        video: None,
        audio_track,
        aac,
        base_us: None,
        last_video_dts: None,
        pending_audio: VecDeque::new(),
        annexb: false,
        audio_started: false,
        aac_frames: 0,
        stats,
        error: None,
    };

    while let Ok(message) = receiver.recv() {
        match message {
            Message::VideoConfig(track) => {
                // Depois de começar, o formato não pode mudar no meio do arquivo.
                if state.video.is_none() {
                    state.annexb = track.description.is_empty();
                    state.video = Some(track);
                }
            }
            Message::Video { data, pts_us, key } => state.on_video(data, pts_us, key),
            Message::Audio { samples, pts_us } => state.on_audio(&samples, pts_us),
            Message::Stop(reply) => {
                let fragment = state.fragmenter.finish();
                state.emit(fragment);
                let result = match (&state.video, state.base_us) {
                    (Some(video), Some(_)) => state.sink.finish(video, state.audio_track.as_ref()).map_err(|e| e.to_string()),
                    _ => Ok(None),
                };
                let result = match state.error.take() {
                    Some(error) => Err(error),
                    None => result,
                };
                let _ = reply.send(result);
                return;
            }
        }
    }
}

/// Converte o codec do WebCodecs (`avc1.640033`, `hvc1.1.6.L153.B0`…) no tipo do MP4.
pub fn parse_codec(codec: &str) -> Option<VideoCodec> {
    if codec.starts_with("avc1") || codec.starts_with("avc3") {
        Some(VideoCodec::Avc)
    } else if codec.starts_with("hvc1") || codec.starts_with("hev1") {
        Some(VideoCodec::Hevc)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    //! Teste de ponta a ponta do muxer com vídeo H.264 de verdade e o AAC do Windows.
    //! Gera o fixture com um encoder qualquer (avcc.bin + frames.bin) e roda:
    //! `CAPTURAX_FIXTURE=<pasta> cargo test --lib mux_fixture -- --ignored`
    use super::*;

    struct DirSink {
        dir: PathBuf,
        count: usize,
    }

    impl FragmentSink for DirSink {
        fn init(&mut self, bytes: &[u8], _moov_offset: usize, _codecs: &str) -> std::io::Result<()> {
            std::fs::write(self.dir.join("init.mp4"), bytes)
        }
        fn fragment(&mut self, fragment: &Fragment) -> std::io::Result<()> {
            self.count += 1;
            std::fs::write(self.dir.join(format!("seg{:03}.m4s", self.count)), &fragment.bytes)
        }
        fn finish(&mut self, _: &VideoTrack, _: Option<&AudioTrack>) -> std::io::Result<Option<PathBuf>> {
            Ok(None)
        }
    }

    /// Transmissão "ao vivo" de verdade: repete o fixture em tempo real numa sessão de baixa
    /// latência e serve em :18787 (`CAPTURAX_FIXTURE=<pasta> CAPTURAX_SERVE_SECS=40`).
    #[test]
    #[ignore]
    fn serve_live_fixture() {
        let dir = PathBuf::from(std::env::var("CAPTURAX_FIXTURE").expect("CAPTURAX_FIXTURE"));
        let secs: u64 = std::env::var("CAPTURAX_SERVE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30);
        let description = std::fs::read(dir.join("avcc.bin")).unwrap();
        let raw = std::fs::read(dir.join("frames.bin")).unwrap();
        let mut frames = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            let size = u32::from_le_bytes(raw[i..i + 4].try_into().unwrap()) as usize;
            let pts = i64::from_le_bytes(raw[i + 4..i + 12].try_into().unwrap());
            frames.push((pts, raw[i + 12] == 1, raw[i + 13..i + 13 + size].to_vec()));
            i += 13 + size;
        }
        let loop_us = frames.last().unwrap().0 + 16_667;

        let server = crate::stream::StreamServer::new();
        tauri::async_runtime::block_on(server.apply(true, 18787, false));
        let (sender, receiver) = mpsc::channel();
        let options = SessionOptions {
            kind: SessionKind::Stream,
            sink: Box::new(crate::stream::LiveSink::new(server.hub.clone())),
            audio: Some(MixSettings { inputs: vec![], system: None, excluded: vec![], offset_ms: 0 }),
            audio_bitrate_kbps: 128,
            fragment_secs: 1.0,
            part_secs: Some(0.1),
        };
        let thread = std::thread::spawn(move || run_session(options, receiver, Arc::new(Stats::default())));
        let track = VideoTrack { codec: VideoCodec::Avc, description, width: 1280, height: 720, codec_string: "avc1.64001f".into() };
        sender.send(Message::VideoConfig(track)).unwrap();

        println!("live on http://127.0.0.1:18787/ for {secs}s");
        let start = clock::now_us();
        let (mut frame, mut loops, mut audio_pts, mut phase) = (0usize, 0i64, start, 0f32);
        while clock::now_us() - start < secs as i64 * 1_000_000 {
            let now = clock::now_us();
            // Vídeo no ritmo real.
            loop {
                let (pts, key, data) = &frames[frame];
                let at = start + loops * loop_us + pts;
                if at > now {
                    break;
                }
                sender.send(Message::Video { data: data.clone(), pts_us: at, key: *key }).unwrap();
                frame += 1;
                if frame == frames.len() {
                    frame = 0;
                    loops += 1;
                }
            }
            // Áudio em blocos de 10 ms.
            while audio_pts + 10_000 <= now {
                let mut block = Vec::with_capacity(crate::audio::BLOCK_FRAMES * CHANNELS);
                for _ in 0..crate::audio::BLOCK_FRAMES {
                    let v = (phase * std::f32::consts::TAU).sin() * 0.2;
                    phase = (phase + 440.0 / SAMPLE_RATE as f32).fract();
                    block.extend([v, v]);
                }
                sender.send(Message::Audio { samples: Arc::new(block), pts_us: audio_pts }).unwrap();
                audio_pts += 10_000;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let (reply, result) = mpsc::channel();
        sender.send(Message::Stop(reply)).unwrap();
        let _ = result.recv();
        thread.join().unwrap();
    }

    #[test]
    #[ignore]
    fn mux_fixture() {
        let dir = PathBuf::from(std::env::var("CAPTURAX_FIXTURE").expect("CAPTURAX_FIXTURE"));
        let description = std::fs::read(dir.join("avcc.bin")).unwrap();
        let raw = std::fs::read(dir.join("frames.bin")).unwrap();
        let mut frames = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            let size = u32::from_le_bytes(raw[i..i + 4].try_into().unwrap()) as usize;
            let pts = i64::from_le_bytes(raw[i + 4..i + 12].try_into().unwrap());
            let key = raw[i + 12] == 1;
            frames.push((pts, key, raw[i + 13..i + 13 + size].to_vec()));
            i += 13 + size;
        }
        let codec = std::fs::read_to_string(dir.join("codec.txt")).ok().and_then(|c| parse_codec(c.trim())).unwrap_or(VideoCodec::Avc);
        let track = VideoTrack { codec, description, width: 1280, height: 720, codec_string: "avc1.64001f".into() };
        let base: i64 = 5_000_000_000;
        let end = frames.last().unwrap().0;

        let hls_dir = dir.join("hls");
        let _ = std::fs::remove_dir_all(&hls_dir);
        std::fs::create_dir_all(&hls_dir).unwrap();
        let sinks: Vec<Box<dyn FragmentSink>> = vec![
            Box::new(FileSink::create(dir.join("out.mp4")).unwrap()),
            Box::new(DirSink { dir: hls_dir, count: 0 }),
        ];

        for sink in sinks {
            let (sender, receiver) = mpsc::channel();
            let options = SessionOptions {
                kind: SessionKind::Recording,
                sink,
                audio: Some(MixSettings { inputs: vec![], system: None, excluded: vec![], offset_ms: 0 }),
                audio_bitrate_kbps: 192,
                fragment_secs: 1.0,
                part_secs: None,
            };
            let thread = std::thread::spawn(move || run_session(options, receiver, Arc::new(Stats::default())));
            sender.send(Message::VideoConfig(track.clone())).unwrap();

            // Áudio começa 200 ms antes do vídeo (precisa ser cortado) e segue até o fim.
            let mut audio_pts = base - 200_000;
            let mut phase = 0f32;
            let mut next_frame = 0;
            while audio_pts < base + end + 100_000 {
                while next_frame < frames.len() && base + frames[next_frame].0 <= audio_pts {
                    let (pts, key, data) = &frames[next_frame];
                    sender.send(Message::Video { data: data.clone(), pts_us: base + pts, key: *key }).unwrap();
                    next_frame += 1;
                }
                let mut block = Vec::with_capacity(crate::audio::BLOCK_FRAMES * CHANNELS);
                for _ in 0..crate::audio::BLOCK_FRAMES {
                    let v = (phase * std::f32::consts::TAU).sin() * 0.3;
                    phase = (phase + 440.0 / SAMPLE_RATE as f32).fract();
                    block.extend([v, v]);
                }
                sender.send(Message::Audio { samples: Arc::new(block), pts_us: audio_pts }).unwrap();
                audio_pts += 10_000;
            }
            let (reply, result) = mpsc::channel();
            sender.send(Message::Stop(reply)).unwrap();
            println!("result: {:?}", result.recv().unwrap());
            thread.join().unwrap();
        }
    }
}
