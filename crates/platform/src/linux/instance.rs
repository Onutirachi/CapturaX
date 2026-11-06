//! Single instance on Linux: an abstract Unix stream socket `\0<name>`; `bind` fails with
//! `AddrInUse` while another instance holds it, and the kernel releases it when the process ends
//! (contracts/single-instance.md).

use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::time::Duration;

use crate::single_instance::TIMEOUT;

fn address(name: &str) -> io::Result<SocketAddr> {
    SocketAddr::from_abstract_name(name.as_bytes())
}

pub struct Listener(UnixListener);

/// Binds the socket; None when another process already has it.
pub fn bind(name: &str) -> io::Result<Option<Listener>> {
    match UnixListener::bind_addr(&address(name)?) {
        Ok(listener) => Ok(Some(Listener(listener))),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => Ok(None),
        Err(e) => Err(e),
    }
}

impl Listener {
    /// Waits for a client; a client that stalls is dropped after the timeout.
    pub fn accept(&self) -> io::Result<UnixStream> {
        let (stream, _) = self.0.accept()?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        Ok(stream)
    }
}

/// Connects to a running instance.
pub fn connect(name: &str, timeout: Duration) -> io::Result<UnixStream> {
    let stream = UnixStream::connect_addr(&address(name)?)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}
