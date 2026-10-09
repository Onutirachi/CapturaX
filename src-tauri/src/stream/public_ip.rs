//! IP público desta conexão, para o link de quem assiste pela internet.
//!
//! Pergunta ao DNS da OpenDNS pelo nome especial `myip.opendns.com`, que responde com o IP de
//! quem perguntou. Só uma consulta DNS (UDP), sem biblioteca extra nem HTTP.

use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;

const RESOLVERS: [&str; 2] = ["208.67.222.222:53", "208.67.220.220:53"];
const NAME: &str = "myip.opendns.com";

fn query(id: u16) -> Vec<u8> {
    let mut packet = Vec::with_capacity(64);
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]); // recursão; 1 pergunta
    for label in NAME.split('.') {
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.extend_from_slice(&[0, 0, 1, 0, 1]); // fim do nome; tipo A; classe IN
    packet
}

/// Lê o primeiro registro A da resposta.
fn parse(response: &[u8], id: u16) -> Option<Ipv4Addr> {
    if response.len() < 12 || u16::from_be_bytes([response[0], response[1]]) != id {
        return None;
    }
    let answers = u16::from_be_bytes([response[6], response[7]]);
    let mut i = 12;
    // Pula a pergunta (nome + tipo + classe).
    while *response.get(i)? != 0 {
        i += 1 + response[i] as usize;
    }
    i += 5;
    for _ in 0..answers {
        // Nome comprimido (2 bytes) ou por extenso.
        if response.get(i)? & 0xc0 == 0xc0 {
            i += 2;
        } else {
            while *response.get(i)? != 0 {
                i += 1 + response[i] as usize;
            }
            i += 1;
        }
        let kind = u16::from_be_bytes([*response.get(i)?, *response.get(i + 1)?]);
        let length = u16::from_be_bytes([*response.get(i + 8)?, *response.get(i + 9)?]) as usize;
        i += 10;
        if kind == 1 && length == 4 {
            return Some(Ipv4Addr::new(response[i], response[i + 1], response[i + 2], response[i + 3]));
        }
        i += length;
    }
    None
}

pub fn lookup() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let id = (crate::clock::now_us() & 0xffff) as u16;
    for resolver in RESOLVERS {
        if socket.send_to(&query(id), resolver).is_err() {
            continue;
        }
        let mut buffer = [0u8; 512];
        if let Ok((length, _)) = socket.recv_from(&mut buffer) {
            if let Some(ip) = parse(&buffer[..length], id) {
                return Some(ip);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn public_ip_lookup() {
        println!("public IP: {:?}", super::lookup());
    }
}
