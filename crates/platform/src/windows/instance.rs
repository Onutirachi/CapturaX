//! Single instance on Windows: a named pipe `\\.\pipe\<name>` created with
//! `FILE_FLAG_FIRST_PIPE_INSTANCE`, so a second creation fails while the first lives
//! (contracts/single-instance.md). One pipe instance serves one connection at a time.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FlushFileBuffers, PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
};

/// Pipe buffers: requests and answers are a few bytes.
const BUFFER: u32 = 512;

fn path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The pipe's only instance, owned as a file so reads and writes go through `std`.
pub struct Listener(File);

/// Creates the pipe; None when another process already has it.
pub fn bind(name: &str) -> io::Result<Option<Listener>> {
    let path = wide(&path(name));
    // SAFETY: `path` is a NUL-terminated UTF-16 string that outlives the call; no security
    // attributes (the default descriptor lets only the owner, SYSTEM and administrators write).
    let handle = unsafe {
        CreateNamedPipeW(
            path.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            BUFFER,
            BUFFER,
            0,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        let error = io::Error::last_os_error();
        return match error.raw_os_error().map(|code| code as u32) {
            // ACCESS_DENIED: the name exists (first-instance flag); PIPE_BUSY: its one instance
            // is taken.
            Some(ERROR_ACCESS_DENIED | ERROR_PIPE_BUSY) => Ok(None),
            _ => Err(error),
        };
    }
    // SAFETY: a valid handle just created, owned by nothing else.
    Ok(Some(Listener(unsafe { File::from_raw_handle(handle) })))
}

impl Listener {
    /// Waits for a client. The connection disconnects it when dropped.
    pub fn accept(&self) -> io::Result<Connection<'_>> {
        // SAFETY: the pipe handle lives as long as `self`; synchronous call, no OVERLAPPED.
        let connected = unsafe { ConnectNamedPipe(self.0.as_raw_handle(), std::ptr::null_mut()) } != 0;
        if !connected {
            let error = io::Error::last_os_error();
            // A client that connected between two calls is connected all the same.
            if error.raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32) {
                return Err(error);
            }
        }
        Ok(Connection(&self.0))
    }
}

/// A connected client on the pipe's instance.
pub struct Connection<'a>(&'a File);

impl Read for Connection<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Connection<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Connection<'_> {
    fn drop(&mut self) {
        let handle = self.0.as_raw_handle();
        // SAFETY: the pipe handle is valid while the listener lives. Flushing first lets the
        // client read the answer before the disconnect discards it.
        unsafe {
            FlushFileBuffers(handle);
            DisconnectNamedPipe(handle);
        }
    }
}

/// Connects to a running instance, waiting up to `timeout` while its pipe serves someone else.
pub fn connect(name: &str, timeout: Duration) -> io::Result<File> {
    let path = path(name);
    let wide_path = wide(&path);
    let deadline = Instant::now() + timeout;
    loop {
        // Identification only: the server cannot act as this process.
        match OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(&path)
        {
            Ok(file) => return Ok(file),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                let ms = u32::try_from(left.as_millis()).unwrap_or(u32::MAX).max(1);
                // SAFETY: NUL-terminated UTF-16 that outlives the call. A failure (timeout, or the
                // pipe gone) shows on the next open.
                unsafe { WaitNamedPipeW(wide_path.as_ptr(), ms) };
            }
            Err(e) => return Err(e),
        }
    }
}
