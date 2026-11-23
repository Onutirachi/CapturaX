//! The clipboard (screenshot FR-001, research R4): a `clipboard` thread that owns one
//! `arboard::Clipboard` for the app's lifetime. On Linux the owner serves the image to the apps
//! that paste it (the X11 selection, or the Wayland data-control protocol), so it must live as
//! long as the image should stay pasteable (spec Edge Cases); on Windows arboard writes
//! `CF_DIBV5` and the registered `PNG` format.

use std::sync::{Arc, mpsc};
use std::time::Duration;

/// One image to put on the clipboard, and where its answer goes.
struct Job {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
    answer: mpsc::Sender<Result<(), String>>,
}

/// The `clipboard` thread. Dropping it lets the thread end once its last job is done.
pub struct Clipboard {
    /// None when the thread could not start.
    jobs: Option<mpsc::Sender<Job>>,
}

impl Clipboard {
    /// Starts the thread; the system clipboard is opened on it at the first image.
    pub fn start() -> Clipboard {
        let (jobs, queue) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("clipboard".into())
            .spawn(move || serve(queue));
        match spawned {
            Ok(_) => Clipboard { jobs: Some(jobs) },
            Err(err) => {
                log::warn!("clipboard: cannot start its thread: {err}");
                Clipboard { jobs: None }
            }
        }
    }

    /// Puts an RGBA image (`width × height`, rows of `width × 4` bytes) on the clipboard; blocks
    /// the caller (the screenshot worker) until done or `timeout`. Err carries a short reason for
    /// the log. An image still being set when `timeout` passes is set anyway, later.
    pub fn set_image(&self, width: u32, height: u32, rgba: Arc<[u8]>, timeout: Duration) -> Result<(), String> {
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(format!("{} bytes for {width}x{height}", rgba.len()));
        }
        let jobs = self.jobs.as_ref().ok_or("no clipboard thread")?;
        let (answer, result) = mpsc::channel();
        jobs.send(Job {
            width,
            height,
            rgba,
            answer,
        })
        .map_err(|_| "clipboard thread gone")?;
        match result.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("timed out after {} ms", timeout.as_millis())),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err("clipboard thread gone".into()),
        }
    }
}

/// The thread's loop: opens the clipboard at the first job (again at the next one when that
/// failed) and keeps it until the [`Clipboard`] is dropped.
fn serve(queue: mpsc::Receiver<Job>) {
    let mut clipboard: Option<arboard::Clipboard> = None;
    for job in queue {
        let result = set(&mut clipboard, &job);
        // The caller may have stopped waiting.
        let _ = job.answer.send(result);
    }
}

fn set(clipboard: &mut Option<arboard::Clipboard>, job: &Job) -> Result<(), String> {
    if clipboard.is_none() {
        match arboard::Clipboard::new() {
            Ok(opened) => *clipboard = Some(opened),
            Err(err) => return Err(format!("no clipboard: {err}")),
        }
    }
    let Some(clipboard) = clipboard.as_mut() else {
        return Err("no clipboard".into());
    };
    clipboard
        .set_image(arboard::ImageData {
            width: job.width as usize,
            height: job.height as usize,
            bytes: std::borrow::Cow::Borrowed(&job.rgba),
        })
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_size_is_refused_before_the_clipboard() {
        let clipboard = Clipboard { jobs: None };
        let rgba: Arc<[u8]> = vec![0u8; 15].into();
        let err = clipboard.set_image(2, 2, rgba, Duration::from_millis(10)).unwrap_err();
        assert_eq!(err, "15 bytes for 2x2");
    }

    #[test]
    fn no_thread_is_an_error_not_a_wait() {
        let clipboard = Clipboard { jobs: None };
        let rgba: Arc<[u8]> = vec![0u8; 16].into();
        assert_eq!(
            clipboard.set_image(2, 2, rgba, Duration::from_secs(5)),
            Err("no clipboard thread".into())
        );
    }

    #[test]
    fn timeout_answers_while_the_thread_is_busy() {
        // A thread that takes the job and never answers in time.
        let (jobs, queue) = mpsc::channel::<Job>();
        let busy = std::thread::spawn(move || {
            let job = queue.recv().unwrap();
            std::thread::sleep(Duration::from_millis(200));
            let _ = job.answer.send(Ok(()));
        });
        let clipboard = Clipboard { jobs: Some(jobs) };
        let rgba: Arc<[u8]> = vec![0u8; 4].into();
        let err = clipboard.set_image(1, 1, rgba, Duration::from_millis(20)).unwrap_err();
        assert_eq!(err, "timed out after 20 ms");
        busy.join().unwrap();
    }
}
