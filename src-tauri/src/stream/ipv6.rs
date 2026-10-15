//! Endereço IPv6 público deste PC, para o link de quem assiste sem abrir porta.
//!
//! Com IPv6 não existe NAT: cada aparelho tem o próprio endereço público, e quem tem IPv6 chega
//! direto ao PC (no máximo o firewall IPv6 do roteador precisa deixar). O Windows cria também
//! endereços temporários (trocados todo dia, para privacidade); o link usa o fixo da interface.

use std::net::Ipv6Addr;

use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{IpDadStatePreferred, IpSuffixOriginRandom, AF_INET6, SOCKADDR_IN6};

/// Global unicast (2000::/3): o único tipo que funciona pela internet.
fn is_global(ip: &Ipv6Addr) -> bool {
    ip.segments()[0] & 0xe000 == 0x2000
}

/// O endereço IPv6 público (fixo, se houver; senão um temporário) das interfaces ligadas.
pub fn public_ipv6() -> Option<Ipv6Addr> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size = 16 * 1024u32;
    let mut buffer: Vec<u64>;
    loop {
        // u64 para o alinhamento das estruturas.
        buffer = vec![0u64; (size as usize).div_ceil(8)];
        let result = unsafe { GetAdaptersAddresses(AF_INET6.0 as u32, flags, None, Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size) };
        if result == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if result != NO_ERROR.0 {
            return None;
        }
        break;
    }

    let (mut stable, mut temporary) = (None, None);
    let mut adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    unsafe {
        while let Some(current) = adapter.as_ref() {
            if current.OperStatus == IfOperStatusUp {
                let mut unicast = current.FirstUnicastAddress;
                while let Some(address) = unicast.as_ref() {
                    let sockaddr = address.Address.lpSockaddr;
                    if !sockaddr.is_null() && (*sockaddr).sa_family == AF_INET6 && address.DadState == IpDadStatePreferred {
                        let ip = Ipv6Addr::from((*(sockaddr as *const SOCKADDR_IN6)).sin6_addr.u.Byte);
                        if is_global(&ip) {
                            if address.SuffixOrigin == IpSuffixOriginRandom {
                                temporary.get_or_insert(ip);
                            } else {
                                stable.get_or_insert(ip);
                            }
                        }
                    }
                    unicast = address.Next;
                }
            }
            adapter = current.Next;
        }
    }
    stable.or(temporary)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn print_public_ipv6() {
        println!("public IPv6: {:?}", super::public_ipv6());
    }
}
