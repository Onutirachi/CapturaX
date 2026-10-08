//! Guarda em memória os últimos segmentos HLS e quem está assistindo.

use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use crate::clock;
use crate::media::fmp4::{AudioTrack, Fragment, VideoTrack};
use crate::media::FragmentSink;

/// Segmentos mantidos (≈ 10 s). O playlist lista os mais recentes.
const KEEP_SEGMENTS: usize = 10;
const PLAYLIST_SEGMENTS: usize = 6;
/// Sem pedir segmento por esse tempo, o espectador conta como saído.
const VIEWER_TIMEOUT: Duration = Duration::from_secs(15);

struct Segment {
    sequence: u64,
    duration: f64,
    data: Arc<Vec<u8>>,
}

#[derive(Default)]
struct Live {
    /// Muda a cada transmissão; vai na URL para o player não misturar segmentos de duas.
    session: u64,
    init: Option<Arc<Vec<u8>>>,
    segments: VecDeque<Segment>,
    next_sequence: u64,
}

struct Viewer {
    first_seen: Instant,
    last_seen: Instant,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ViewerInfo {
    pub ip: String,
    pub watching_secs: u64,
}

#[derive(Default)]
pub struct StreamHub {
    live: Mutex<Live>,
    viewers: Mutex<HashMap<IpAddr, Viewer>>,
    blocked: RwLock<HashSet<IpAddr>>,
    pub max_viewers: AtomicU32,
}

impl StreamHub {
    pub fn new() -> Arc<Self> {
        let hub = Arc::new(Self::default());
        hub.max_viewers.store(10, Ordering::Relaxed);
        hub
    }

    fn begin(&self) {
        let mut live = self.live.lock().unwrap();
        *live = Live { session: clock::now_us() as u64, ..Default::default() };
    }

    fn end(&self) {
        *self.live.lock().unwrap() = Live::default();
    }

    /// Ao vivo = já tem o cabeçalho e pelo menos um segmento.
    pub fn status(&self) -> (bool, u64) {
        let live = self.live.lock().unwrap();
        (live.init.is_some() && !live.segments.is_empty(), live.session)
    }

    pub fn playlist(&self, session: u64) -> Option<String> {
        let live = self.live.lock().unwrap();
        if live.session != session || live.init.is_none() || live.segments.is_empty() {
            return None;
        }
        let window: Vec<&Segment> = live.segments.iter().rev().take(PLAYLIST_SEGMENTS).collect::<Vec<_>>().into_iter().rev().collect();
        let target = window.iter().map(|s| s.duration.ceil() as u64).max().unwrap_or(1).max(2);
        let mut text = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:{}\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"init.mp4\"\n",
            window[0].sequence
        );
        for segment in window {
            text.push_str(&format!("#EXTINF:{:.3},\n{}.m4s\n", segment.duration, segment.sequence));
        }
        Some(text)
    }

    pub fn init(&self, session: u64) -> Option<Arc<Vec<u8>>> {
        let live = self.live.lock().unwrap();
        (live.session == session).then(|| live.init.clone()).flatten()
    }

    pub fn segment(&self, session: u64, sequence: u64) -> Option<Arc<Vec<u8>>> {
        let live = self.live.lock().unwrap();
        if live.session != session {
            return None;
        }
        live.segments.iter().find(|s| s.sequence == sequence).map(|s| s.data.clone())
    }

    pub fn is_blocked(&self, ip: IpAddr) -> bool {
        self.blocked.read().unwrap().contains(&ip)
    }

    pub fn set_blocked(&self, ips: &[String]) {
        let parsed = ips.iter().filter_map(|ip| ip.parse().ok()).collect();
        *self.blocked.write().unwrap() = parsed;
        let blocked = self.blocked.read().unwrap();
        self.viewers.lock().unwrap().retain(|ip, _| !blocked.contains(ip));
    }

    /// Registra o espectador; `false` se a sala estiver cheia.
    pub fn touch_viewer(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut viewers = self.viewers.lock().unwrap();
        viewers.retain(|_, v| now.duration_since(v.last_seen) < VIEWER_TIMEOUT);
        if let Some(viewer) = viewers.get_mut(&ip) {
            viewer.last_seen = now;
            return true;
        }
        if viewers.len() >= self.max_viewers.load(Ordering::Relaxed) as usize {
            return false;
        }
        viewers.insert(ip, Viewer { first_seen: now, last_seen: now });
        true
    }

    pub fn viewers(&self) -> Vec<ViewerInfo> {
        let now = Instant::now();
        let mut viewers = self.viewers.lock().unwrap();
        viewers.retain(|_, v| now.duration_since(v.last_seen) < VIEWER_TIMEOUT);
        viewers.iter().map(|(ip, v)| ViewerInfo { ip: ip.to_string(), watching_secs: now.duration_since(v.first_seen).as_secs() }).collect()
    }
}

/// Destino da sessão de transmissão: os fragmentos viram segmentos HLS em memória.
pub struct HlsSink {
    hub: Arc<StreamHub>,
}

impl HlsSink {
    pub fn new(hub: Arc<StreamHub>) -> Self {
        hub.begin();
        Self { hub }
    }
}

impl FragmentSink for HlsSink {
    fn init(&mut self, bytes: &[u8], _moov_offset: usize) -> std::io::Result<()> {
        self.hub.live.lock().unwrap().init = Some(Arc::new(bytes.to_vec()));
        Ok(())
    }

    fn fragment(&mut self, fragment: &Fragment) -> std::io::Result<()> {
        let mut live = self.hub.live.lock().unwrap();
        let sequence = live.next_sequence;
        live.next_sequence += 1;
        live.segments.push_back(Segment { sequence, duration: fragment.duration_secs, data: Arc::new(fragment.bytes.clone()) });
        while live.segments.len() > KEEP_SEGMENTS {
            live.segments.pop_front();
        }
        Ok(())
    }

    fn finish(&mut self, _video: &VideoTrack, _audio: Option<&AudioTrack>) -> std::io::Result<Option<PathBuf>> {
        self.hub.end();
        Ok(None)
    }
}

impl Drop for HlsSink {
    fn drop(&mut self) {
        self.hub.end();
    }
}
