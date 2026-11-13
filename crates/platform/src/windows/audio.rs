//! Audio devices and streams on Windows (WASAPI); T017 and T018 replace it. Until then every call reports
//! `Failed("not implemented")`.

use std::sync::Arc;

use crate::audio::{AudioError, OnAudioEvent, OnSessionEvent, Samples, SessionEvent, SessionRequest, Side};

fn not_implemented() -> AudioError {
    AudioError::Failed("not implemented".into())
}

pub struct Monitor;

impl Monitor {
    pub fn start(_on_event: OnAudioEvent) -> Result<Self, AudioError> {
        Err(not_implemented())
    }
}

pub fn video_link(_video_id: &str) -> Option<String> {
    None
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
