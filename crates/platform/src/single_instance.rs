//! Single instance (FR-013, contracts/single-instance.md): one running instance per settings
//! location. The first start claims a local channel named from the location and listens on a
//! thread; a later start finds the name taken, sends `show` and exits.
//!
//! The channel is a named pipe on Windows and an abstract Unix socket on Linux (research R10);
//! this module holds the shared part: the name, the protocol and the listener thread.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::os::instance as os;

/// How long a second start waits for the running instance's answer, and how long the running
/// instance waits for a request line once connected (where the OS allows it).
pub const TIMEOUT: Duration = Duration::from_secs(1);

/// The only request the listener accepts.
const SHOW: &[u8] = b"show";

/// Longest line read before giving up on a connection: longer than any valid line.
const MAX_LINE: usize = 32;

/// The channel name for a settings location (or, with settings in memory, the executable):
/// `capturax-` and the FNV-1a 64-bit hash of the resolved path as 16 hex digits.
///
/// The path is resolved first (its folder made canonical, so a file that does not exist yet
/// gives the same name as once it does), and on Windows compared without case, so two starts
/// through differently written paths share one instance.
pub fn instance_name(path: &Path) -> String {
    let resolved = resolve(path);
    let text = resolved.to_string_lossy();
    let text = if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.into_owned()
    };
    format!("capturax-{:016x}", fnv1a(text.as_bytes()))
}

fn resolve(path: &Path) -> PathBuf {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    match (parent, path.file_name()) {
        (Some(parent), Some(name)) => match parent.canonicalize() {
            Ok(parent) => parent.join(name),
            Err(_) => path.to_path_buf(),
        },
        _ => path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
    }
}

/// FNV-1a, 64 bits: a few lines, and stable across Rust versions (unlike `DefaultHasher`).
fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes
        .iter()
        .fold(OFFSET, |hash, &b| (hash ^ u64::from(b)).wrapping_mul(PRIME))
}

/// The running instance's answer to `show`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The window was shown and brought to the front.
    Raised,
    /// The window was shown, but the platform did not let it take focus (Wayland).
    ShownNotRaised,
}

impl Answer {
    fn line(self) -> &'static [u8] {
        match self {
            Answer::Raised => b"raised\n",
            Answer::ShownNotRaised => b"shown-not-raised\n",
        }
    }

    fn parse(line: &[u8]) -> Option<Self> {
        match line {
            b"raised" => Some(Answer::Raised),
            b"shown-not-raised" => Some(Answer::ShownNotRaised),
            _ => None,
        }
    }
}

/// The result of claiming the instance name.
pub enum Claim {
    /// This is the only instance: start listening with [`Listener::serve`].
    First(Listener),
    /// Another instance runs: ask it to show its window with [`Client::show`].
    Running(Client),
}

/// Claims the instance `name` (from [`instance_name`]). An error means the channel could not be
/// created for another reason than a running instance.
pub fn claim_instance(name: &str) -> io::Result<Claim> {
    Ok(match os::bind(name)? {
        Some(listener) => Claim::First(Listener {
            name: name.to_owned(),
            os: listener,
        }),
        None => Claim::Running(Client { name: name.to_owned() }),
    })
}

/// The claimed name, not listening yet.
pub struct Listener {
    name: String,
    os: os::Listener,
}

impl Listener {
    /// Listens on a thread until the returned [`Serving`] drops. For each `show` it calls
    /// `on_show`, which shows the window (through the event loop) and returns whether the
    /// platform lets it come to the front ([`crate::can_raise_window`]).
    pub fn serve(self, on_show: impl Fn() -> bool + Send + 'static) -> io::Result<Serving> {
        let stop = Arc::new(AtomicBool::new(false));
        let (done_tx, done) = mpsc::channel();
        let listener = self.os;
        let thread_stop = Arc::clone(&stop);
        thread::Builder::new().name("single-instance".into()).spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok(_) if thread_stop.load(Ordering::Acquire) => break,
                    Ok(mut conn) => {
                        if let Err(e) = serve_one(&mut conn, &on_show) {
                            log::debug!("single instance: connection dropped: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("single instance: stopped listening: {e}");
                        break;
                    }
                }
            }
            drop(listener);
            let _ = done_tx.send(());
        })?;
        Ok(Serving {
            name: self.name,
            stop,
            done,
        })
    }
}

/// Answers one connection: reads one line and answers it, or closes the connection on anything
/// but `show`.
fn serve_one(conn: &mut (impl Read + Write), on_show: &dyn Fn() -> bool) -> io::Result<()> {
    let Some(line) = read_line(conn)? else {
        return Ok(());
    };
    if line != SHOW {
        log::warn!("single instance: unknown request ignored");
        return Ok(());
    }
    log::info!("single instance: another start asked to show the window");
    let answer = if on_show() {
        Answer::Raised
    } else {
        Answer::ShownNotRaised
    };
    conn.write_all(answer.line())?;
    conn.flush()
}

/// Reads up to a newline. None when the line is too long, is not finished, or the peer closed
/// first.
fn read_line(conn: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::with_capacity(MAX_LINE);
    let mut byte = [0u8; 1];
    while line.len() < MAX_LINE {
        if conn.read(&mut byte)? == 0 {
            return Ok(None);
        }
        if byte[0] == b'\n' {
            return Ok(Some(line));
        }
        line.push(byte[0]);
    }
    Ok(None)
}

/// The listening instance. Dropping it stops the thread and releases the name (quit sequence).
pub struct Serving {
    name: String,
    stop: Arc<AtomicBool>,
    done: mpsc::Receiver<()>,
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wakes the thread from its accept; it then sees the flag and closes the channel.
        drop(os::connect(&self.name, TIMEOUT));
        if self.done.recv_timeout(TIMEOUT).is_err() {
            log::warn!("single instance: listener did not stop; released at exit");
        }
    }
}

/// A connection to the running instance.
pub struct Client {
    name: String,
}

impl Client {
    /// Sends `show` and waits up to [`TIMEOUT`] for the answer. None when there was no valid
    /// answer in time (the caller logs it and exits all the same).
    pub fn show(&self) -> Option<Answer> {
        let (tx, rx) = mpsc::channel();
        let name = self.name.clone();
        // A thread, so the wait is bounded on Windows too, where pipe reads have no timeout.
        let spawned = thread::Builder::new()
            .name("single-instance-client".into())
            .spawn(move || {
                let _ = tx.send(exchange(&name));
            });
        if let Err(e) = spawned {
            log::warn!("single instance: {e}");
            return None;
        }
        match rx.recv_timeout(TIMEOUT) {
            Ok(Ok(answer)) => answer,
            Ok(Err(e)) => {
                log::warn!("single instance: no answer from the running instance: {e}");
                None
            }
            Err(_) => {
                log::warn!("single instance: no answer from the running instance in time");
                None
            }
        }
    }
}

fn exchange(name: &str) -> io::Result<Option<Answer>> {
    let mut conn = os::connect(name, TIMEOUT)?;
    conn.write_all(SHOW)?;
    conn.write_all(b"\n")?;
    conn.flush()?;
    Ok(read_line(&mut conn)?.and_then(|line| Answer::parse(&line)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One side of a connection in memory: reads `input`, collects what is written.
    struct Fake {
        input: io::Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Fake {
        fn new(input: &[u8]) -> Self {
            Self {
                input: io::Cursor::new(input.to_vec()),
                output: Vec::new(),
            }
        }
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn answer_to(input: &[u8], raised: bool) -> (Vec<u8>, bool) {
        let called = std::cell::Cell::new(false);
        let mut conn = Fake::new(input);
        serve_one(&mut conn, &|| {
            called.set(true);
            raised
        })
        .unwrap();
        (conn.output, called.get())
    }

    #[test]
    fn fnv1a_matches_the_reference_values() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn name_is_prefix_and_sixteen_hex_digits() {
        let name = instance_name(Path::new("/no/such/folder/capturax.toml"));
        let hash = name.strip_prefix("capturax-").unwrap();
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn missing_file_and_existing_file_share_a_name() {
        let dir = tempfile_dir();
        let file = dir.join("capturax.toml");
        let before = instance_name(&file);
        std::fs::write(&file, "").unwrap();
        assert_eq!(instance_name(&file), before);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn different_folders_have_different_names() {
        assert_ne!(
            instance_name(Path::new("/one/capturax.toml")),
            instance_name(Path::new("/two/capturax.toml"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_names_ignore_case() {
        assert_eq!(
            instance_name(Path::new(r"D:\No\Such\capturax.toml")),
            instance_name(Path::new(r"d:\no\such\CAPTURAX.toml"))
        );
    }

    #[test]
    fn show_is_answered_from_the_callback() {
        assert_eq!(answer_to(b"show\n", true), (b"raised\n".to_vec(), true));
        assert_eq!(answer_to(b"show\n", false), (b"shown-not-raised\n".to_vec(), true));
    }

    #[test]
    fn anything_else_closes_without_acting() {
        for input in [
            &b"show"[..],
            b"",
            b"Show\n",
            b"show \n",
            b"show\r\n",
            b"quit\n",
            &[b'x'; 100],
        ] {
            assert_eq!(answer_to(input, true), (Vec::new(), false), "{input:?}");
        }
    }

    #[test]
    fn answers_parse() {
        assert_eq!(Answer::parse(b"raised"), Some(Answer::Raised));
        assert_eq!(Answer::parse(b"shown-not-raised"), Some(Answer::ShownNotRaised));
        assert_eq!(Answer::parse(b"ok"), None);
        for answer in [Answer::Raised, Answer::ShownNotRaised] {
            let line = answer.line();
            assert_eq!(Answer::parse(&line[..line.len() - 1]), Some(answer));
        }
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("capturax-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
