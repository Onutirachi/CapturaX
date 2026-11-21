//! Audio devices and streams on Linux (PipeWire; preview audio research R3, R5, R6). The monitor
//! is a watcher of the `pipewire` thread ([`super::pipewire`]) and the session is two streams on it;
//! both only send it commands, so no call here waits for a device (research R9).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::pipewire::{self, Command};
use crate::audio::{AudioError, OnAudioEvent, OnSessionEvent, Samples, SessionEvent, SessionRequest, Side};

/// The running monitor: a watcher of the `pipewire` thread, removed when this is dropped.
pub struct Monitor {
    watcher: u64,
}

impl Monitor {
    /// Starts the `pipewire` thread when it is not running, waiting only for its connection to
    /// PipeWire; the first list follows as an event.
    pub fn start(on_event: OnAudioEvent) -> Result<Self, AudioError> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let watcher = NEXT.fetch_add(1, Ordering::Relaxed);
        pipewire::send(Command::Watch(watcher, on_event))?;
        Ok(Self { watcher })
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        pipewire::post(Command::Unwatch(self.watcher));
    }
}

/// A bus path as research R5 compares it: a USB path without its trailing `:<n>.<n>` (the
/// interface, which differs between a device's video and audio); other paths (a PCI function such
/// as `pci-0000:00:1f.3`) are kept whole.
pub(super) fn usb_key(bus_path: &str) -> &str {
    let Some((head, tail)) = bus_path.rsplit_once(':') else {
        return bus_path;
    };
    let is_interface = tail
        .split_once('.')
        .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit()));
    if is_interface && head.contains("-usb-") {
        head
    } else {
        bus_path
    }
}

/// The key of a V4L2 device, from the bus path its id ends with
/// (`v4l2:<vid>:<pid>:<serial>@<bus path>`). None for other ids.
pub fn video_link(video_id: &str) -> Option<String> {
    let (_, bus_path) = video_id.strip_prefix("v4l2:")?.rsplit_once('@')?;
    (!bus_path.is_empty()).then(|| usb_key(bus_path).to_owned())
}

/// A session on the `pipewire` thread: two streams there, known by the session's number.
pub struct Session {
    number: u64,
    on_event: OnSessionEvent,
    /// The `pipewire` thread took the session; false when PipeWire could not be reached.
    sent: bool,
}

impl Session {
    pub fn open(request: SessionRequest, samples: Arc<dyn Samples>, on_event: OnSessionEvent) -> Self {
        let number = request.number;
        let command = Command::Open {
            request,
            samples,
            on_event: Arc::clone(&on_event),
        };
        let mut session = Self {
            number,
            on_event,
            sent: true,
        };
        if let Err(error) = pipewire::send(command) {
            session.sent = false;
            session.report(vec![
                SessionEvent::Error {
                    number,
                    side: Side::Input,
                    error: error.clone(),
                },
                SessionEvent::Error {
                    number,
                    side: Side::Output,
                    error,
                },
            ]);
        }
        session
    }

    /// The exclusive flag is ignored: PipeWire has no such mode.
    pub fn reopen_input(&self, input: String, _exclusive: bool) {
        self.reopen(Side::Input, Some(input));
    }

    pub fn reopen_output(&self, output: Option<String>, _exclusive: bool) {
        self.reopen(Side::Output, output);
    }

    pub fn close(&self) {
        if self.sent {
            pipewire::post(Command::Close(self.number));
        } else {
            self.report(vec![SessionEvent::Closed { number: self.number }]);
        }
    }

    fn reopen(&self, side: Side, target: Option<String>) {
        let number = self.number;
        if self.sent {
            pipewire::post(Command::Reopen { number, side, target });
        } else {
            let error = AudioError::Failed("PipeWire is not running".into());
            self.report(vec![SessionEvent::Error { number, side, error }]);
        }
    }

    /// Reports from another thread, as the `pipewire` thread would; for a session it never took.
    fn report(&self, events: Vec<SessionEvent>) {
        let on_event = Arc::clone(&self.on_event);
        std::thread::spawn(move || events.into_iter().for_each(|event| on_event(event)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_usb_interface_suffix_is_removed() {
        assert_eq!(usb_key("pci-0000:03:00.0-usb-0:3:1.2"), "pci-0000:03:00.0-usb-0:3");
        assert_eq!(usb_key("pci-0000:03:00.0-usb-0:3"), "pci-0000:03:00.0-usb-0:3");
        assert_eq!(usb_key("pci-0000:00:1f.3"), "pci-0000:00:1f.3");
        assert_eq!(usb_key("platform-snd_aloop.0"), "platform-snd_aloop.0");
    }

    #[test]
    fn a_video_and_its_audio_give_one_key() {
        // The ezcap on CachyOS (measurements/pairing.md): video interface 1.0, audio 1.2.
        let video = video_link("v4l2:32ed:3201:ezcap_ezcap_Live_GAMER_ULTRA_00000001@pci-0000:03:00.0-usb-0:3:1.0");
        assert_eq!(video.as_deref(), Some(usb_key("pci-0000:03:00.0-usb-0:3:1.2")));
    }

    #[test]
    fn only_v4l2_ids_have_a_video_link() {
        assert_eq!(video_link("mf:usb#vid_32ed&pid_3201"), None);
        assert_eq!(video_link("v4l2:32ed:3201:x"), None);
        assert_eq!(video_link("v4l2:32ed:3201:x@"), None);
        assert_eq!(video_link("test"), None);
    }
}
