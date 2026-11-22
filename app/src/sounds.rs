//! The sound player (screenshot research R10, data-model.md "Sound player"): where the app's event
//! sounds play. A sound plays on the output chosen for the preview audio: mixed into the preview
//! session's output when that one is open, otherwise through an output-only session on the same
//! output, opened on the first play and closed [`IDLE_CLOSE`] after the sound ends. A play waits at
//! most [`OPEN_WAIT`] for an output that is opening (FR-010), then it is dropped and logged.
//!
//! [`Router`] makes the decisions and is pure (tested here); [`Sounds`] carries them out with the
//! platform session. [`crate::sound::Sound`] owns the player and tells it how the preview's output
//! is; a session of the player that closes holds a new preview session back as an older preview
//! session does, so a device the preview asks exclusive is free.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ::audio::resample;
use ::audio::sound::{self, Decoded, Prepared, Sound as Effect, Voice};
use platform::audio::{Session, SessionEvent, SessionRequest};
use winit::event_loop::EventLoopProxy;

use crate::app::UserEvent;
use crate::sound::{Closed, Closing, Feed};

/// The capture sound, made by the maintainer (screenshot tasks T040); later sounds come from the
/// same instrument and pattern.
const SCREENSHOT_WAV: &[u8] = include_bytes!("../sounds/screenshot.wav");

/// The longest a play waits for an output that is opening (FR-010).
pub const OPEN_WAIT: Duration = Duration::from_millis(300);
/// How long an output-only session stays open after its sound ends.
pub const IDLE_CLOSE: Duration = Duration::from_secs(3);

/// How the preview session's output side is, as [`crate::sound::Sound`] sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewOutput {
    /// No preview session, or its output side failed and nothing reopens it.
    Closed,
    /// The session or its output side is opening (open, queued or reopening).
    Opening,
    /// The output side opened: its `fill` runs.
    Open,
}

/// What the router asks of the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Play on the preview session's output.
    PlayPreview,
    /// Play on the output-only session's output.
    PlayOwn,
    /// Open the output-only session `number` on `output` (None: the system default), shared.
    Open { number: u64, output: Option<String> },
    /// Close the output-only session `number`.
    Close { number: u64 },
    /// A play waited [`OPEN_WAIT`] and no output opened: dropped.
    Dropped,
}

/// The output-only session as the router knows it.
#[derive(Debug)]
struct Own {
    number: u64,
    output: Option<String>,
    opened: bool,
    /// When it closes (set once it opened, moved at each play).
    close_at: Option<Instant>,
}

/// A play waiting for an output.
#[derive(Debug)]
struct Pending {
    deadline: Instant,
    /// The output resolved at the play, for an output-only session.
    output: Option<String>,
}

/// Where a play goes (data-model.md "Sound player"). Pure: the caller passes the time and runs
/// the [`Action`]s.
#[derive(Debug)]
pub struct Router {
    preview: PreviewOutput,
    own: Option<Own>,
    pending: Option<Pending>,
    /// Output-only session numbers handed out so far.
    sessions: u64,
    /// The sound's length, for the close after it ends.
    length: Duration,
}

impl Router {
    pub fn new(length: Duration) -> Self {
        Self {
            preview: PreviewOutput::Closed,
            own: None,
            pending: None,
            sessions: 0,
            length,
        }
    }

    /// A sound asked, for `output` as the preview audio resolves it now. A new play replaces one
    /// still waiting (the voice restarts anyway).
    pub fn play(&mut self, output: Option<String>, now: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        // The output choice changed since the session opened: it opens on the new one.
        if self.preview == PreviewOutput::Closed && self.own.as_ref().is_some_and(|own| own.output != output) {
            self.close_own(&mut out);
        }
        self.pending = Some(Pending {
            deadline: now + OPEN_WAIT,
            output,
        });
        self.settle(now, &mut out);
        out
    }

    /// The preview's output changed state. While the preview opens or plays, the output-only
    /// session closes: the preview takes over its sounds.
    pub fn preview(&mut self, state: PreviewOutput, now: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        self.preview = state;
        if state != PreviewOutput::Closed {
            self.close_own(&mut out);
        }
        self.settle(now, &mut out);
        out
    }

    /// The output-only session `number` opened.
    pub fn opened(&mut self, number: u64, now: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(own) = self.own.as_mut().filter(|own| own.number == number) {
            own.opened = true;
            own.close_at = Some(now + IDLE_CLOSE);
        }
        self.settle(now, &mut out);
        out
    }

    /// The output-only session `number` failed: it closes, and a play waiting for it is dropped
    /// (opening again at once would fail the same way).
    pub fn failed(&mut self, number: u64) -> Vec<Action> {
        let mut out = Vec::new();
        if self.own.as_ref().is_some_and(|own| own.number == number) {
            self.close_own(&mut out);
            if self.preview == PreviewOutput::Closed && self.pending.take().is_some() {
                out.push(Action::Dropped);
            }
        }
        out
    }

    /// The output-only session `number` closed without being asked: forgotten.
    pub fn closed(&mut self, number: u64) {
        if self.own.as_ref().is_some_and(|own| own.number == number) {
            self.own = None;
        }
    }

    /// Drops a play that waited too long and closes an idle session.
    pub fn tick(&mut self, now: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        self.settle(now, &mut out);
        out
    }

    /// When [`Router::tick`] has something to do.
    pub fn next_wake(&self) -> Option<Instant> {
        let deadline = self.pending.as_ref().map(|p| p.deadline);
        let close = self.own.as_ref().filter(|own| own.opened).and_then(|own| own.close_at);
        deadline.into_iter().chain(close).min()
    }

    /// The quit: every session closes, a waiting play is forgotten.
    pub fn quit(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        self.pending = None;
        self.close_own(&mut out);
        out
    }

    fn close_own(&mut self, out: &mut Vec<Action>) {
        if let Some(own) = self.own.take() {
            out.push(Action::Close { number: own.number });
        }
    }

    fn settle(&mut self, now: Instant, out: &mut Vec<Action>) {
        if self.pending.as_ref().is_some_and(|p| now >= p.deadline) {
            self.pending = None;
            out.push(Action::Dropped);
        }
        if let Some(pending) = &self.pending {
            match (self.preview, &mut self.own) {
                (PreviewOutput::Open, _) => {
                    self.pending = None;
                    out.push(Action::PlayPreview);
                }
                (PreviewOutput::Opening, _) | (PreviewOutput::Closed, Some(Own { opened: false, .. })) => {}
                (PreviewOutput::Closed, Some(own)) => {
                    own.close_at = Some(now + self.length + IDLE_CLOSE);
                    self.pending = None;
                    out.push(Action::PlayOwn);
                }
                (PreviewOutput::Closed, None) => {
                    self.sessions += 1;
                    let output = pending.output.clone();
                    self.own = Some(Own {
                        number: self.sessions,
                        output: output.clone(),
                        opened: false,
                        close_at: None,
                    });
                    out.push(Action::Open {
                        number: self.sessions,
                        output,
                    });
                }
            }
        }
        if self.pending.is_none()
            && self
                .own
                .as_ref()
                .is_some_and(|own| own.opened && own.close_at.is_some_and(|at| now >= at))
        {
            self.close_own(out);
        }
    }
}

/// The preview session's output as the player plays on it: its feed, and the format its `Opened`
/// said (used until the feed's first `fill` tells the real one).
pub type PreviewFeed<'a> = Option<(&'a Feed, Option<resample::Format>)>;

/// The open output-only session.
struct OwnSession {
    number: u64,
    session: Session,
    feed: Arc<Feed>,
    closed: Arc<Closed>,
    /// The format its `Opened` said.
    opened: Option<resample::Format>,
}

/// The sound player: the router, the sound, and the output-only session.
pub struct Sounds {
    router: Router,
    /// None when the embedded file does not read (logged at start): no sound plays.
    screenshot: Option<Decoded>,
    /// The sound converted to the last output format it played on.
    prepared: Option<Prepared>,
    own: Option<OwnSession>,
    proxy: EventLoopProxy<UserEvent>,
}

impl Sounds {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let screenshot = match sound::decode(SCREENSHOT_WAV) {
            Ok(decoded) => Some(decoded),
            Err(err) => {
                log::error!("screenshot sound unreadable: {err}");
                None
            }
        };
        let length = screenshot.as_ref().map_or(Duration::ZERO, |decoded| {
            let frames = decoded.samples.len() / usize::from(decoded.format.channels);
            Duration::from_secs_f64(frames as f64 / f64::from(decoded.format.rate))
        });
        Self {
            router: Router::new(length),
            screenshot,
            prepared: None,
            own: None,
            proxy,
        }
    }

    /// Plays `effect` on `output` (as the preview audio resolves it). Returns the sessions it
    /// asked to close.
    pub fn play(&mut self, effect: Effect, output: Option<String>, preview: PreviewFeed<'_>) -> Vec<Closing> {
        match effect {
            Effect::Screenshot if self.screenshot.is_none() => return Vec::new(),
            Effect::Screenshot => {}
        }
        let actions = self.router.play(output, Instant::now());
        self.run(actions, preview)
    }

    /// The preview's output changed state.
    pub fn preview(&mut self, state: PreviewOutput, preview: PreviewFeed<'_>) -> Vec<Closing> {
        let actions = self.router.preview(state, Instant::now());
        self.run(actions, preview)
    }

    /// An event of an output-only session.
    pub fn event(&mut self, event: SessionEvent, preview: PreviewFeed<'_>) -> Vec<Closing> {
        let now = Instant::now();
        let actions = match event {
            SessionEvent::Opened { number, format, .. } => {
                log::info!("sound session {number}: opened {format}");
                if let Some(own) = self.own.as_mut().filter(|own| own.number == number) {
                    own.opened = Some(resample::Format {
                        rate: format.rate,
                        channels: format.channels,
                    });
                }
                self.router.opened(number, now)
            }
            SessionEvent::Error { number, error, .. } => {
                log::warn!("sound session {number}: {error}");
                self.router.failed(number)
            }
            SessionEvent::Closed { number } => {
                if self.own.as_ref().is_some_and(|own| own.number == number) {
                    self.own = None;
                }
                self.router.closed(number);
                Vec::new()
            }
        };
        self.run(actions, preview)
    }

    pub fn tick(&mut self, now: Instant, preview: PreviewFeed<'_>) -> Vec<Closing> {
        let actions = self.router.tick(now);
        self.run(actions, preview)
    }

    pub fn next_wake(&self) -> Option<Instant> {
        self.router.next_wake()
    }

    /// The quit: the output-only session closes.
    pub fn quit(&mut self) -> Vec<Closing> {
        let actions = self.router.quit();
        self.run(actions, None)
    }

    fn run(&mut self, actions: Vec<Action>, preview: PreviewFeed<'_>) -> Vec<Closing> {
        let mut closing = Vec::new();
        for action in actions {
            match action {
                Action::PlayPreview => match preview {
                    Some((feed, opened)) => self.play_on(feed, opened),
                    None => log::warn!("sound: the preview output is gone"),
                },
                Action::PlayOwn => {
                    if let Some(own) = &self.own {
                        let (feed, opened) = (Arc::clone(&own.feed), own.opened);
                        self.play_on(&feed, opened);
                    }
                }
                Action::Open { number, output } => self.open(number, output),
                Action::Close { number } => {
                    if let Some(mut own) = self.own.take_if(|own| own.number == number) {
                        own.session.close();
                        closing.push(Closing {
                            number,
                            own: true,
                            closed: own.closed,
                            asked: Instant::now(),
                        });
                    }
                }
                Action::Dropped => log::warn!("sound: no output opened within {OPEN_WAIT:?}; dropped"),
            }
        }
        closing
    }

    fn open(&mut self, number: u64, output: Option<String>) {
        log::info!("sound session {number}: open {output:?}");
        let feed = Arc::new(Feed::new(1.0));
        let closed = Arc::new(Closed::default());
        let on_closed = Arc::clone(&closed);
        let proxy = self.proxy.clone();
        let request = SessionRequest {
            number,
            input: None,
            output,
            input_exclusive: false,
            output_exclusive: false,
        };
        let session = Session::open(request, Arc::clone(&feed) as _, move |event| {
            if matches!(event, SessionEvent::Closed { .. }) {
                on_closed.set();
            }
            let _ = proxy.send_event(UserEvent::SoundSession(event));
        });
        self.own = Some(OwnSession {
            number,
            session,
            feed,
            closed,
            opened: None,
        });
    }

    /// Plays the sound on `feed`, prepared for the format its output runs in (the last `fill`'s,
    /// else `opened`); a voice of another format is replaced.
    fn play_on(&mut self, feed: &Feed, opened: Option<resample::Format>) {
        let Some(decoded) = &self.screenshot else { return };
        let Some(format) = feed.output_format().or(opened) else {
            log::warn!("sound: the output's format is unknown; dropped");
            return;
        };
        let voice = match feed.voice() {
            Some(voice) if voice.format() == format => voice,
            _ => {
                let prepared = match &self.prepared {
                    Some(prepared) if prepared.format == format => prepared.clone(),
                    _ => {
                        let prepared = sound::prepare(decoded, format);
                        self.prepared = Some(prepared.clone());
                        prepared
                    }
                };
                let voice = Arc::new(Voice::new(prepared));
                feed.set_voice(Arc::clone(&voice));
                voice
            }
        };
        voice.play();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTH: Duration = Duration::from_millis(120);
    const HEADSET: &str = "headset";

    fn open(number: u64, output: Option<&str>) -> Action {
        Action::Open {
            number,
            output: output.map(str::to_owned),
        }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn plays_on_the_preview_when_its_output_is_open() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        assert!(router.preview(PreviewOutput::Open, t).is_empty());
        assert_eq!(router.play(None, t), [Action::PlayPreview]);
        assert_eq!(router.next_wake(), None);
    }

    #[test]
    fn without_the_preview_an_output_only_session_opens_plays_and_closes_after_the_sound() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        assert_eq!(router.play(Some(HEADSET.into()), t), [open(1, Some(HEADSET))]);
        assert_eq!(router.next_wake(), Some(t + OPEN_WAIT));
        assert_eq!(router.opened(1, t + ms(20)), [Action::PlayOwn]);
        let close = t + ms(20) + LENGTH + IDLE_CLOSE;
        assert_eq!(router.next_wake(), Some(close));
        // A second play keeps the session and moves its close.
        assert_eq!(router.play(Some(HEADSET.into()), t + ms(1000)), [Action::PlayOwn]);
        let close = t + ms(1000) + LENGTH + IDLE_CLOSE;
        assert!(router.tick(close - ms(1)).is_empty());
        assert_eq!(router.tick(close), [Action::Close { number: 1 }]);
        assert_eq!(router.next_wake(), None);
        // The next play opens a new one.
        assert_eq!(
            router.play(Some(HEADSET.into()), close + ms(1)),
            [open(2, Some(HEADSET))]
        );
    }

    #[test]
    fn a_play_waits_for_the_opening_session_at_most_the_open_wait() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        assert_eq!(router.play(None, t), [open(1, None)]);
        // A second play while it opens neither opens another nor plays yet.
        assert!(router.play(None, t + ms(100)).is_empty());
        assert!(router.tick(t + ms(399)).is_empty());
        assert_eq!(router.tick(t + ms(400)), [Action::Dropped]);
        // Opened late: nothing plays; it closes after the idle time.
        assert!(router.opened(1, t + ms(500)).is_empty());
        assert_eq!(router.next_wake(), Some(t + ms(500) + IDLE_CLOSE));
        assert_eq!(router.tick(t + ms(500) + IDLE_CLOSE), [Action::Close { number: 1 }]);
    }

    #[test]
    fn the_preview_opening_closes_the_output_only_session_and_takes_its_play() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.play(None, t);
        assert_eq!(
            router.preview(PreviewOutput::Opening, t + ms(10)),
            [Action::Close { number: 1 }]
        );
        // An Opened of the closed session changes nothing.
        assert!(router.opened(1, t + ms(20)).is_empty());
        assert_eq!(router.preview(PreviewOutput::Open, t + ms(50)), [Action::PlayPreview]);
    }

    #[test]
    fn a_play_waits_for_the_preview_output_opening_or_reopening() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.preview(PreviewOutput::Opening, t);
        assert!(router.play(None, t).is_empty());
        assert_eq!(router.tick(t + OPEN_WAIT), [Action::Dropped]);
        assert!(router.preview(PreviewOutput::Open, t + ms(400)).is_empty());
    }

    #[test]
    fn the_preview_closing_with_a_waiting_play_opens_an_output_only_session() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.preview(PreviewOutput::Opening, t);
        router.play(Some(HEADSET.into()), t);
        assert_eq!(
            router.preview(PreviewOutput::Closed, t + ms(50)),
            [open(1, Some(HEADSET))]
        );
        assert_eq!(router.opened(1, t + ms(80)), [Action::PlayOwn]);
    }

    #[test]
    fn another_output_chosen_reopens_the_output_only_session() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.play(None, t);
        router.opened(1, t);
        assert_eq!(
            router.play(Some(HEADSET.into()), t + ms(500)),
            [Action::Close { number: 1 }, open(2, Some(HEADSET))]
        );
    }

    #[test]
    fn a_failed_session_closes_and_drops_its_waiting_play() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.play(None, t);
        assert_eq!(router.failed(1), [Action::Close { number: 1 }, Action::Dropped]);
        assert_eq!(router.next_wake(), None);
        // An error of an older session changes nothing.
        router.play(None, t + ms(10));
        assert!(router.failed(1).is_empty());
    }

    #[test]
    fn a_session_closed_by_itself_is_forgotten() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.play(None, t);
        router.opened(1, t);
        router.closed(1);
        assert_eq!(router.play(None, t + ms(10)), [open(2, None)]);
    }

    #[test]
    fn the_quit_closes_the_session() {
        let mut router = Router::new(LENGTH);
        let t = Instant::now();
        router.play(None, t);
        assert_eq!(router.quit(), [Action::Close { number: 1 }]);
        assert_eq!(router.next_wake(), None);
    }

    #[test]
    fn the_embedded_sound_reads() {
        let decoded = sound::decode(SCREENSHOT_WAV).unwrap();
        assert!(!decoded.samples.is_empty());
    }
}
