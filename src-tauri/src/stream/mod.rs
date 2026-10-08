//! Servidor da transmissão por link (HLS), rodando no próprio PC.
//!
//! Superfície mínima de propósito: só GET em rotas fixas, tudo servido da memória (nada é lido do
//! disco a partir da URL), nenhuma rota controla o app, e há limites de conexões, de tamanho de
//! cabeçalho e de tempo. O vídeo é público para quem tiver o link; o PC não fica exposto.

pub mod firewall;
mod hub;
pub mod upnp;

pub use hub::{HlsSink, StreamHub, ViewerInfo};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, Request, Response, StatusCode};
use axum::routing::get;
use axum::{Extension, Router};
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower::ServiceExt;
use upnp::UpnpStatus;

/// Página de quem assiste (gerada por `npm run build:viewer`).
#[derive(RustEmbed)]
#[folder = "viewer-dist/"]
#[allow_missing = true]
struct ViewerAssets;

const MAX_CONNECTIONS: usize = 128;
const MAX_CONNECTIONS_PER_IP: usize = 12;
const CONNECTION_LIFETIME: Duration = Duration::from_secs(300);
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HEADER_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy)]
struct ClientIp(IpAddr);

#[derive(Default)]
struct Connections {
    total: AtomicUsize,
    per_ip: Mutex<HashMap<IpAddr, usize>>,
}

/// Libera a vaga da conexão quando ela termina.
struct ConnectionSlot {
    connections: Arc<Connections>,
    ip: IpAddr,
}

impl Connections {
    fn acquire(self: &Arc<Self>, ip: IpAddr) -> Option<ConnectionSlot> {
        let mut per_ip = self.per_ip.lock().unwrap();
        let count = per_ip.entry(ip).or_default();
        if *count >= MAX_CONNECTIONS_PER_IP || self.total.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
            return None;
        }
        *count += 1;
        self.total.fetch_add(1, Ordering::Relaxed);
        Some(ConnectionSlot { connections: self.clone(), ip })
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        let mut per_ip = self.connections.per_ip.lock().unwrap();
        if let Some(count) = per_ip.get_mut(&self.ip) {
            *count -= 1;
            if *count == 0 {
                per_ip.remove(&self.ip);
            }
        }
        self.connections.total.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
    pub local_url: Option<String>,
    pub public_url: Option<String>,
    pub upnp: UpnpStatus,
    pub firewall_ok: bool,
    pub live: bool,
    pub viewers: Vec<ViewerInfo>,
    pub error: Option<String>,
}

struct Running {
    port: u16,
    upnp: bool,
    shutdown: watch::Sender<bool>,
    task: tauri::async_runtime::JoinHandle<()>,
}

pub struct StreamServer {
    pub hub: Arc<StreamHub>,
    running: tokio::sync::Mutex<Option<Running>>,
    upnp_status: Arc<Mutex<UpnpStatus>>,
    firewall_ok: AtomicBool,
    error: Mutex<Option<String>>,
}

impl StreamServer {
    pub fn new() -> Self {
        Self {
            hub: StreamHub::new(),
            running: tokio::sync::Mutex::new(None),
            upnp_status: Arc::new(Mutex::new(UpnpStatus::Disabled)),
            firewall_ok: AtomicBool::new(false),
            error: Mutex::new(None),
        }
    }

    pub fn refresh_firewall(&self) {
        self.firewall_ok.store(firewall::rule_exists(), Ordering::Relaxed);
    }

    /// Deixa o servidor no estado pedido: liga, desliga, ou reinicia se porta/UPnP mudaram.
    pub async fn apply(&self, enabled: bool, port: u16, upnp: bool) {
        let mut running = self.running.lock().await;
        if let Some(current) = running.as_ref() {
            if enabled && current.port == port && current.upnp == upnp {
                return;
            }
        }
        if let Some(current) = running.take() {
            let _ = current.shutdown.send(true);
            let _ = current.task.await;
        }
        *self.upnp_status.lock().unwrap() = UpnpStatus::Disabled;
        *self.error.lock().unwrap() = None;
        if !enabled {
            return;
        }

        let listener = match TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await {
            Ok(listener) => listener,
            Err(error) => {
                *self.error.lock().unwrap() = Some(format!("Port {port} is not available ({error})"));
                return;
            }
        };
        // IPv6 ajuda quem está atrás de CGNAT no IPv4; se não der, segue só com IPv4.
        let listener_v6 = TcpListener::bind(SocketAddr::from(([0u16; 8], port))).await.ok();
        self.refresh_firewall();

        let (shutdown, shutdown_rx) = watch::channel(false);
        let hub = self.hub.clone();
        let upnp_status = self.upnp_status.clone();
        let task = tauri::async_runtime::spawn(async move {
            let router = router(hub.clone());
            let connections = Arc::new(Connections::default());
            let mut accepts = Vec::new();
            for listener in std::iter::once(listener).chain(listener_v6) {
                accepts.push(tokio::spawn(accept_loop(listener, router.clone(), hub.clone(), connections.clone(), shutdown_rx.clone())));
            }
            let mapping = if upnp { Some(tokio::spawn(manage_upnp(port, upnp_status, shutdown_rx.clone()))) } else { None };
            for accept in accepts {
                let _ = accept.await;
            }
            if let Some(mapping) = mapping {
                let _ = mapping.await;
            }
        });
        *running = Some(Running { port, upnp, shutdown, task });
    }

    pub async fn status(&self) -> ServerStatus {
        let running = self.running.lock().await;
        let port = running.as_ref().map_or(0, |r| r.port);
        let upnp = self.upnp_status.lock().unwrap().clone();
        let (live, _) = self.hub.status();
        let public_url = match &upnp {
            UpnpStatus::Mapped { external_ip, cgnat: false } => Some(format!("http://{external_ip}:{port}/")),
            _ => None,
        };
        ServerStatus {
            running: running.is_some(),
            port,
            local_url: running.as_ref().and_then(|_| lan_ip()).map(|ip| format!("http://{ip}:{port}/")),
            public_url,
            upnp,
            firewall_ok: self.firewall_ok.load(Ordering::Relaxed),
            live,
            viewers: self.hub.viewers(),
            error: self.error.lock().unwrap().clone(),
        }
    }
}

/// IP desta máquina na rede local. O `connect` de UDP só escolhe a rota; nenhum pacote sai.
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

async fn manage_upnp(port: u16, status: Arc<Mutex<UpnpStatus>>, mut shutdown: watch::Receiver<bool>) {
    *status.lock().unwrap() = UpnpStatus::Searching;
    let mapping = match upnp::map(port).await {
        Ok((mapping, mapped)) => {
            *status.lock().unwrap() = mapped;
            mapping
        }
        Err(reason) => {
            *status.lock().unwrap() = UpnpStatus::Failed { reason };
            return;
        }
    };
    loop {
        tokio::select! {
            _ = tokio::time::sleep(upnp::RENEW_EVERY) => {
                if let Err(error) = mapping.renew().await {
                    eprintln!("UPnP renewal failed: {error}");
                }
            }
            _ = shutdown.changed() => break,
        }
    }
    mapping.remove().await;
}

async fn accept_loop(listener: TcpListener, router: Router, hub: Arc<StreamHub>, connections: Arc<Connections>, mut shutdown: watch::Receiver<bool>) {
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        let (stream, address) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                Err(_) => continue,
            },
            _ = shutdown.changed() => break,
        };
        let ip = canonical_ip(address.ip());
        if hub.is_blocked(ip) {
            continue;
        }
        let Some(slot) = connections.acquire(ip) else { continue };
        let router = router.clone();
        tasks.spawn(async move {
            let _slot = slot;
            let _ = stream.set_nodelay(true);
            let service = hyper::service::service_fn(move |mut request: Request<Incoming>| {
                request.extensions_mut().insert(ClientIp(ip));
                router.clone().oneshot(request)
            });
            let connection = hyper::server::conn::http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(HEADER_TIMEOUT)
                .max_buf_size(MAX_HEADER_BYTES)
                .serve_connection(TokioIo::new(stream), service);
            let _ = tokio::time::timeout(CONNECTION_LIFETIME, connection).await;
        });
        // Limpa tarefas terminadas sem bloquear.
        while tasks.try_join_next().is_some() {}
    }
    tasks.abort_all();
}

/// `::ffff:1.2.3.4` (IPv4 via socket IPv6) vira `1.2.3.4`, para bloqueio e contagem baterem.
fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        ip => ip,
    }
}

fn router(hub: Arc<StreamHub>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/live/status", get(status))
        .route("/live/{session}/index.m3u8", get(playlist))
        .route("/live/{session}/init.mp4", get(init_segment))
        .route("/live/{session}/{file}", get(media_segment))
        .fallback(|| async { respond(StatusCode::NOT_FOUND, "text/plain", "Not found", false) })
        .with_state(hub)
}

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
                   media-src 'self' blob:; worker-src 'self' blob:; connect-src 'self'; font-src 'self'; \
                   frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

fn respond(status: StatusCode, content_type: &str, body: impl Into<Body>, cacheable: bool) -> Response<Body> {
    let mut response = Response::new(body.into());
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert("Cross-Origin-Resource-Policy", HeaderValue::from_static("same-origin"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(if cacheable { "public, max-age=60" } else { "no-store" }));
    if content_type.starts_with("text/html") {
        headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    }
    response
}

fn embedded(path: &str) -> Response<Body> {
    match ViewerAssets::get(path) {
        Some(file) => {
            let mime = file.metadata.mimetype().to_string();
            // Arquivos em assets/ têm hash no nome; o HTML sempre revalida.
            respond(StatusCode::OK, &mime, file.data.into_owned(), path.starts_with("assets/"))
        }
        None if path == "index.html" => respond(StatusCode::SERVICE_UNAVAILABLE, "text/plain", "Viewer page not built (run npm run build:viewer)", false),
        None => respond(StatusCode::NOT_FOUND, "text/plain", "Not found", false),
    }
}

async fn index() -> Response<Body> {
    embedded("index.html")
}

async fn asset(Path(path): Path<String>) -> Response<Body> {
    embedded(&format!("assets/{path}"))
}

async fn status(State(hub): State<Arc<StreamHub>>) -> Response<Body> {
    let (live, session) = hub.status();
    respond(StatusCode::OK, "application/json", format!("{{\"live\":{live},\"session\":\"{session}\"}}"), false)
}

fn full() -> Response<Body> {
    respond(StatusCode::SERVICE_UNAVAILABLE, "text/plain", "Too many viewers", false)
}

async fn playlist(State(hub): State<Arc<StreamHub>>, Extension(ClientIp(ip)): Extension<ClientIp>, Path(session): Path<u64>) -> Response<Body> {
    if !hub.touch_viewer(ip) {
        return full();
    }
    match hub.playlist(session) {
        Some(text) => respond(StatusCode::OK, "application/vnd.apple.mpegurl", text, false),
        None => respond(StatusCode::NOT_FOUND, "text/plain", "Offline", false),
    }
}

async fn init_segment(State(hub): State<Arc<StreamHub>>, Path(session): Path<u64>) -> Response<Body> {
    match hub.init(session) {
        Some(data) => respond(StatusCode::OK, "video/mp4", data.as_ref().clone(), true),
        None => respond(StatusCode::NOT_FOUND, "text/plain", "Offline", false),
    }
}

async fn media_segment(State(hub): State<Arc<StreamHub>>, Extension(ClientIp(ip)): Extension<ClientIp>, Path((session, file)): Path<(u64, String)>) -> Response<Body> {
    let Some(sequence) = file.strip_suffix(".m4s").and_then(|n| n.parse::<u64>().ok()) else {
        return respond(StatusCode::NOT_FOUND, "text/plain", "Not found", false);
    };
    if !hub.touch_viewer(ip) {
        return full();
    }
    match hub.segment(session, sequence) {
        Some(data) => respond(StatusCode::OK, "video/iso.segment", data.as_ref().clone(), true),
        None => respond(StatusCode::NOT_FOUND, "text/plain", "Not found", false),
    }
}
