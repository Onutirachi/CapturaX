//! Abre a porta no roteador via UPnP IGD e mantém o mapeamento renovado.

use igd_next::aio::tokio::{search_gateway, Tokio};
use igd_next::aio::Gateway;
use igd_next::{PortMappingProtocol, SearchOptions};
use serde::Serialize;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Duration;

const LEASE_SECS: u32 = 3600;
const DESCRIPTION: &str = "CapturaX";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "state")]
pub enum UpnpStatus {
    Disabled,
    Searching,
    /// Porta aberta; `cgnat` = o IP externo do roteador não é público (a porta não chega da internet).
    Mapped { external_ip: String, cgnat: bool },
    Failed { reason: String },
}

pub struct Mapping {
    gateway: Gateway<Tokio>,
    port: u16,
}

/// IP desta máquina na rede do roteador.
fn local_ip_towards(gateway: SocketAddr) -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(gateway).ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

/// 100.64.0.0/10 (CGNAT) ou faixa privada: o roteador está atrás de outro NAT da operadora.
pub fn is_cgnat(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private() || (a == 100 && (64..128).contains(&b)) || v4.is_loopback() || v4.is_unspecified()
        }
        IpAddr::V6(_) => false,
    }
}

pub async fn map(port: u16) -> Result<(Mapping, UpnpStatus), String> {
    let options = SearchOptions { timeout: Some(Duration::from_secs(5)), ..Default::default() };
    let gateway = search_gateway(options).await.map_err(|e| crate::error::with_detail("upnp_no_router", e))?;
    let local_ip = local_ip_towards(gateway.addr).ok_or_else(|| crate::error::code("no_local_ip"))?;
    gateway
        .add_port(PortMappingProtocol::TCP, port, SocketAddr::new(local_ip, port), LEASE_SECS, DESCRIPTION)
        .await
        .map_err(|e| crate::error::with_detail("upnp_refused", e))?;
    let external_ip = gateway.get_external_ip().await.map_err(|e| crate::error::with_detail("upnp_no_external_ip", e))?;
    let status = UpnpStatus::Mapped { external_ip: external_ip.to_string(), cgnat: is_cgnat(external_ip) };
    Ok((Mapping { gateway, port }, status))
}

impl Mapping {
    /// Renova antes do lease expirar.
    pub async fn renew(&self) -> Result<(), String> {
        let local_ip = local_ip_towards(self.gateway.addr).ok_or_else(|| crate::error::code("no_local_ip"))?;
        self.gateway
            .add_port(PortMappingProtocol::TCP, self.port, SocketAddr::new(local_ip, self.port), LEASE_SECS, DESCRIPTION)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn remove(&self) {
        let _ = self.gateway.remove_port(PortMappingProtocol::TCP, self.port).await;
    }
}

pub const RENEW_EVERY: Duration = Duration::from_secs(LEASE_SECS as u64 / 2);
