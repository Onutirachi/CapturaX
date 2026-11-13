//! Audio devices and streams on Windows (WASAPI; preview audio research R2, R5, R6).
//!
//! - [`endpoints`]: the monitor (`IMMDeviceEnumerator` and an `IMMNotificationClient`).
//! - [`container`]: the container id that links a video device to its audio (pairing).
//! - [`format`]: the system's wave formats in the platform's terms.
//!
//! The session is a stub until T018: every open reports `Failed("not implemented")`.

mod container;
mod endpoints;
mod format;

use std::sync::Arc;

pub use container::video_link;
pub use endpoints::Monitor;

use crate::audio::{AudioError, OnSessionEvent, Samples, SessionEvent, SessionRequest, Side};

fn not_implemented() -> AudioError {
    AudioError::Failed("not implemented".into())
}

pub struct Session {
    number: u64,
    on_event: OnSessionEvent,
}

impl Session {
    pub fn open(request: SessionRequest, _samples: Arc<dyn Samples>, on_event: OnSessionEvent) -> Self {
        let session = Self {
            number: request.number,
            on_event,
        };
        session.fail(Side::Input);
        session.fail(Side::Output);
        session
    }

    pub fn reopen_input(&self, _input: String, _exclusive: bool) {
        self.fail(Side::Input);
    }

    pub fn reopen_output(&self, _output: Option<String>, _exclusive: bool) {
        self.fail(Side::Output);
    }

    pub fn close(&self) {
        let (number, on_event) = (self.number, Arc::clone(&self.on_event));
        std::thread::spawn(move || on_event(SessionEvent::Closed { number }));
    }

    /// Reports the side's error from another thread, as the real streams will.
    fn fail(&self, side: Side) {
        let (number, on_event) = (self.number, Arc::clone(&self.on_event));
        std::thread::spawn(move || {
            on_event(SessionEvent::Error {
                number,
                side,
                error: not_implemented(),
            });
        });
    }
}
