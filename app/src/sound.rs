//! Runs the preview audio's commands (plan "Startup and quit", research R9): the platform's audio
//! monitor and session, and the bridge between the session's two sides. The decisions are
//! [`AudioController`]'s; this module only carries them out, and nothing here waits for an audio
//! thread except the quit, at most [`CLOSE_WAIT`]. A session asked to open while an older one is
//! still closing is queued until that one's `Closed` (at most [`CLOSE_WAIT`]), so a device the
//! older one held exclusive is free (data-model.md "Audio session").

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use ::audio::bridge::Bridge;
use ::audio::resample;
use platform::audio::{Format, Monitor, Samples, Session, SessionEvent, SessionRequest};
use winit::event_loop::EventLoopProxy;

use crate::app::UserEvent;
use crate::audio::{AudioController, Command, Event, VideoDevice};
use crate::stats::AudioSide;

/// The longest the quit waits for the audio session's `Closed` (research R9).
const CLOSE_WAIT: Duration = Duration::from_secs(1);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The formats the session's sides call [`Samples`] with, and the bridge made for them.
#[derive(Default)]
struct Formats {
    input: Option<resample::Format>,
    output: Option<resample::Format>,
    bridge: Option<Arc<Bridge>>,
}

/// One session's [`Samples`]: the input's samples go into the bridge, the output's come out of it.
///
/// The bridge is made from the formats of the calls themselves, not from `Opened` (on Linux that
/// is the device node's format, research R3), once both sides have called; it is made again when
/// a side's format changes or the input reopens ([`Feed::restart_input`]). Until then the input's
/// samples are dropped and the output plays silence. Making it allocates on the audio thread,
/// once per bridge.
struct Feed {
    formats: Mutex<Formats>,
    /// The gain (`f32` bits), given to each new bridge.
    gain: AtomicU32,
}

impl Feed {
    fn new(gain: f32) -> Self {
        Self {
            formats: Mutex::default(),
            gain: AtomicU32::new(gain.to_bits()),
        }
    }

    /// The bridge for a call of `side` in `format`, made when both formats are known.
    fn bridge(&self, format: Format, input: bool) -> Option<Arc<Bridge>> {
        let format = resample::Format {
            rate: format.rate,
            channels: format.channels,
        };
        let mut formats = lock(&self.formats);
        let side = if input { &mut formats.input } else { &mut formats.output };
        if *side != Some(format) {
            *side = Some(format);
            formats.bridge = None;
        }
        if formats.bridge.is_none()
            && let (Some(input), Some(output)) = (formats.input, formats.output)
        {
            let bridge = Bridge::new(input, output);
            bridge.set_gain(f32::from_bits(self.gain.load(Ordering::Relaxed)));
            formats.bridge = Some(Arc::new(bridge));
        }
        formats.bridge.clone()
    }

    /// The bridge playing now, if both sides have called.
    fn current(&self) -> Option<Arc<Bridge>> {
        lock(&self.formats).bridge.clone()
    }

    /// The input reopens: the next bridge starts empty with the new input's format.
    fn restart_input(&self) {
        let mut formats = lock(&self.formats);
        formats.input = None;
        formats.bridge = None;
    }

    fn set_gain(&self, gain: f32) {
        self.gain.store(gain.to_bits(), Ordering::Relaxed);
        if let Some(bridge) = &lock(&self.formats).bridge {
            bridge.set_gain(gain);
        }
    }
}

impl Samples for Feed {
    fn on_input(&self, samples: &[f32], format: Format) {
        if let Some(bridge) = self.bridge(format, true) {
            let frames = samples.len() / usize::from(format.channels.max(1));
            bridge.set_input_period(u32::try_from(frames).unwrap_or(u32::MAX));
            bridge.push(samples);
        }
    }

    fn fill(&self, out: &mut [f32], format: Format) {
        match self.bridge(format, false) {
            Some(bridge) => bridge.pull(out),
            None => out.fill(0.0),
        }
    }
}

/// Set once a session reported `Closed`.
#[derive(Default)]
struct Closed {
    done: Mutex<bool>,
    signal: Condvar,
}

impl Closed {
    fn set(&self) {
        *lock(&self.done) = true;
        self.signal.notify_all();
    }

    /// Waits until set or `deadline`; true when set.
    fn wait_until(&self, deadline: Instant) -> bool {
        let mut done = lock(&self.done);
        while !*done {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            done = self
                .signal
                .wait_timeout(done, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }
}

/// The open platform session.
struct Open {
    number: u64,
    session: Session,
    feed: Arc<Feed>,
    closed: Arc<Closed>,
    /// The endpoints asked for (None: the system default output), as the last open or reopen said.
    input_id: String,
    output_id: Option<String>,
    /// Each side as its last `Opened` said, for the `[audio-source]` line.
    input: Option<AudioSide>,
    output: Option<AudioSide>,
}

/// A session waiting for the older ones to close, with what was asked of it meanwhile.
#[derive(Debug, Clone, PartialEq)]
struct Queued {
    request: SessionRequest,
    gain: f32,
}

impl Queued {
    /// Applies a reopen or a gain asked while queued (an open or a close replaces or drops it).
    fn apply(&mut self, command: &Command) {
        match command {
            Command::ReopenInput { input, exclusive } => {
                self.request.input = Some(input.clone());
                self.request.input_exclusive = *exclusive;
            }
            Command::ReopenOutput { output, exclusive } => {
                self.request.output.clone_from(output);
                self.request.output_exclusive = *exclusive;
            }
            Command::SetGain(gain) => self.gain = *gain,
            Command::Open(_) | Command::Close => {}
        }
    }
}

/// When the sessions asked to close at `asked` stop holding a new one back: each one's `Closed`,
/// or [`CLOSE_WAIT`] after it was asked. None: nothing holds it back.
fn held_until(asked: impl Iterator<Item = Instant>) -> Option<Instant> {
    asked.map(|at| at + CLOSE_WAIT).max()
}

/// The `--stats` lines' clock (contracts/stats.md).
struct StatsClock {
    /// Where `t` counts from: the video thread's start, so `[audio] t` matches `[loop] t`.
    origin: Instant,
    /// When the next `[audio]` line is due; None while no session is open.
    next: Option<Instant>,
}

/// The preview's sound in the app: the controller, the monitor and the open session.
pub struct Sound {
    controller: AudioController,
    monitor: Option<Monitor>,
    open: Option<Open>,
    /// The sessions asked to close whose `Closed` has not come: number, flag, when asked.
    closing: Vec<(u64, Arc<Closed>, Instant)>,
    /// The session that opens once `closing` is empty or has waited [`CLOSE_WAIT`].
    queued: Option<Queued>,
    /// Some with `--stats`.
    stats: Option<StatsClock>,
    proxy: EventLoopProxy<UserEvent>,
}

impl Sound {
    /// `stats`: print the `--stats` lines.
    pub fn new(proxy: EventLoopProxy<UserEvent>, stats: bool) -> Self {
        Self {
            controller: AudioController::new(),
            monitor: None,
            open: None,
            closing: Vec::new(),
            queued: None,
            stats: stats.then(|| StatsClock {
                origin: Instant::now(),
                next: None,
            }),
            proxy,
        }
    }

    /// What the Audio select shows (its menu, value and the open video device).
    pub fn controller(&self) -> &AudioController {
        &self.controller
    }

    /// The video thread started at `origin`: the `[audio]` line's `t` counts from there, as the
    /// `[loop]` line's.
    pub fn stats_origin(&mut self, origin: Instant) {
        if let Some(clock) = &mut self.stats {
            clock.origin = origin;
        }
    }

    /// When the next `[audio]` line is due (`--stats` and a session open).
    pub fn next_stats(&self) -> Option<Instant> {
        self.stats.as_ref().and_then(|clock| clock.next)
    }

    /// Prints the `[audio]` line once its second has passed, with what the bridge did in it
    /// (`wait_ms=-/-` while no bridge plays).
    pub fn print_stats(&mut self, now: Instant) {
        let (Some(clock), Some(open)) = (&mut self.stats, &self.open) else {
            return;
        };
        if clock.next.is_none_or(|next| now < next) {
            return;
        }
        clock.next = Some(now + Duration::from_secs(1));
        let t = now.saturating_duration_since(clock.origin).as_secs();
        let stats = open.feed.current().map(|bridge| bridge.take_stats());
        crate::render::print_stats(&crate::stats::audio_line(t, stats));
    }

    /// A side of the open session opened: kept for the `[audio-source]` line, printed with
    /// `--stats` once both sides have opened (again at each reopen).
    fn side_opened(&mut self, event: &SessionEvent) {
        let SessionEvent::Opened {
            number,
            side,
            mode,
            format,
            period_ms,
            reported_latency_ms,
        } = event
        else {
            return;
        };
        // Events of an older session are dropped (the open one is the controller's).
        if self.controller.session() != Some(*number) {
            return;
        }
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let id = match side {
            platform::audio::Side::Input => Some(open.input_id.as_str()),
            platform::audio::Side::Output => open.output_id.as_deref(),
        };
        let name = self
            .controller
            .endpoint_name(id)
            .or(id)
            .unwrap_or("system default")
            .to_owned();
        let opened = Some(AudioSide {
            name,
            mode: *mode,
            format: *format,
            period_ms: *period_ms,
            latency_ms: *reported_latency_ms,
        });
        match side {
            platform::audio::Side::Input => open.input = opened,
            platform::audio::Side::Output => open.output = opened,
        }
        if self.stats.is_some()
            && let (Some(input), Some(output)) = (&open.input, &open.output)
        {
            crate::render::print_stats(&crate::stats::audio_source(open.number, input, output));
        }
    }

    /// Starts the audio monitor; its events come back as [`UserEvent::Audio`]. Without it the
    /// video plays without sound.
    pub fn start(&mut self) {
        let proxy = self.proxy.clone();
        match Monitor::start(move |event| {
            let _ = proxy.send_event(UserEvent::Audio(Event::Monitor(event)));
        }) {
            Ok(monitor) => self.monitor = Some(monitor),
            Err(err) => log::error!("audio monitor unavailable: {err}"),
        }
    }

    /// A video session opened on `device` (preview states Opening, Playing, No signal follow).
    pub fn video_opened(&mut self, session: u64, device: &capture::Device, settings: &settings::Settings) {
        let device = VideoDevice {
            id: device.id.clone(),
            base_name: device.base_name.clone(),
            link: platform::audio::video_link(&device.id),
        };
        log::info!("video device {} audio link {:?}", device.id, device.link);
        self.handle(Event::VideoOpened { session, device }, settings);
    }

    pub fn video_closed(&mut self, session: u64, settings: &settings::Settings) {
        self.handle(Event::VideoClosed { session }, settings);
    }

    /// Hands an event to the controller and runs its commands; a state change is logged with the
    /// event that caused it.
    pub fn handle(&mut self, event: Event, settings: &settings::Settings) {
        match &event {
            Event::Session(SessionEvent::Closed { number }) => {
                if let Some(at) = self.closing.iter().position(|(closing, ..)| closing == number) {
                    let (_, _, asked) = self.closing.swap_remove(at);
                    // SC-008: every close under 1 s, read from this line (quickstart).
                    log::info!(
                        "audio session {number} closed in {:.0} ms",
                        asked.elapsed().as_secs_f64() * 1e3
                    );
                }
            }
            Event::Session(event @ SessionEvent::Opened { .. }) => {
                log::info!("audio: {event:?}");
                self.side_opened(event);
            }
            _ => {}
        }
        let reason = event.reason();
        let before = self.controller.state();
        let commands = self.controller.handle(event, settings);
        let after = self.controller.state();
        if after != before {
            log::info!("audio state {before:?} -> {after:?} ({reason})");
        }
        for command in commands {
            self.run(command, settings);
        }
        self.open_queued(Instant::now());
    }

    /// When the queued session opens even if the older ones have not closed; None: nothing queued.
    pub fn next_open(&self) -> Option<Instant> {
        self.queued.as_ref()?;
        held_until(self.closing.iter().map(|(.., asked)| *asked))
    }

    /// Opens the queued session once no older one is closing, or once they had [`CLOSE_WAIT`].
    pub fn open_queued(&mut self, now: Instant) {
        if self.queued.is_none() {
            return;
        }
        if self.closing.is_empty() {
            let queued = self.queued.take().expect("checked");
            self.open_now(queued);
        } else if held_until(self.closing.iter().map(|(.., asked)| *asked)).is_some_and(|at| now >= at) {
            let queued = self.queued.take().expect("checked");
            log::warn!(
                "audio session {}: an older session did not close within {CLOSE_WAIT:?}; opening anyway",
                queued.request.number
            );
            self.open_now(queued);
        }
    }

    fn run(&mut self, command: Command, settings: &settings::Settings) {
        match command {
            Command::Open(request) => {
                self.close();
                let gain = ::audio::volume::gain(settings.audio.volume, settings.audio.muted);
                let queued = Queued { request, gain };
                // A close that never came holds nothing back after CLOSE_WAIT.
                let held = held_until(self.closing.iter().map(|(.., asked)| *asked));
                if held.is_none_or(|at| Instant::now() >= at) {
                    self.open_now(queued);
                } else {
                    log::info!(
                        "audio session {}: waits for session(s) {:?} to close",
                        queued.request.number,
                        self.closing.iter().map(|(number, ..)| *number).collect::<Vec<_>>()
                    );
                    self.queued = Some(queued);
                }
            }
            command @ (Command::ReopenInput { .. } | Command::ReopenOutput { .. } | Command::SetGain(_))
                if self.queued.is_some() =>
            {
                self.queued.as_mut().expect("checked").apply(&command);
            }
            Command::ReopenInput { input, exclusive } => {
                if let Some(open) = &mut self.open {
                    open.feed.restart_input();
                    open.input_id.clone_from(&input);
                    open.session.reopen_input(input, exclusive);
                }
            }
            Command::ReopenOutput { output, exclusive } => {
                if let Some(open) = &mut self.open {
                    open.output_id.clone_from(&output);
                    open.session.reopen_output(output, exclusive);
                }
            }
            Command::Close => self.close(),
            Command::SetGain(gain) => {
                if let Some(open) = &self.open {
                    open.feed.set_gain(gain);
                }
            }
        }
    }

    /// Opens the session now (no older one holds its devices).
    fn open_now(&mut self, queued: Queued) {
        let Queued { request, gain } = queued;
        let number = request.number;
        let feed = Arc::new(Feed::new(gain));
        let closed = Arc::new(Closed::default());
        let proxy = self.proxy.clone();
        log::info!(
            "audio session {number}: open {:?} -> {:?}",
            request.input,
            request.output
        );
        let on_closed = Arc::clone(&closed);
        let (input_id, output_id) = (request.input.clone().unwrap_or_default(), request.output.clone());
        let session = Session::open(request, Arc::clone(&feed) as Arc<dyn Samples>, move |event| {
            // Set here, not on the main thread: the quit waits for it there.
            if matches!(event, SessionEvent::Closed { .. }) {
                on_closed.set();
            }
            let _ = proxy.send_event(UserEvent::Audio(Event::Session(event)));
        });
        self.open = Some(Open {
            number,
            session,
            feed,
            closed,
            input_id,
            output_id,
            input: None,
            output: None,
        });
        if let Some(clock) = &mut self.stats {
            clock.next = Some(Instant::now() + Duration::from_secs(1));
        }
    }

    /// Asks the open session to close and drops a queued one; returns at once (its `Closed` comes
    /// later).
    fn close(&mut self) {
        self.queued = None;
        if let Some(mut open) = self.open.take() {
            open.session.close();
            self.closing.push((open.number, open.closed, Instant::now()));
        }
        if let Some(clock) = &mut self.stats {
            clock.next = None;
        }
    }

    /// Quit (research R9): the open session closes and the audio threads get at most
    /// [`CLOSE_WAIT`] to release their devices; then the monitor stops. A close that does not come
    /// is logged and the quit goes on (the process exits after the settings are saved).
    pub fn quit(&mut self) {
        self.close();
        let deadline = Instant::now() + CLOSE_WAIT;
        let started = Instant::now();
        for (number, closed, _) in self.closing.drain(..) {
            if closed.wait_until(deadline) {
                log::info!(
                    "audio session {number} closed {:.0} ms into the quit",
                    started.elapsed().as_secs_f64() * 1e3
                );
            } else {
                log::warn!("audio session {number} did not close within {CLOSE_WAIT:?}; quitting anyway");
            }
        }
        self.monitor = None;
    }
}

#[cfg(test)]
mod tests {
    use platform::audio::SampleFormat;

    use super::*;

    fn format(rate: u32, channels: u16) -> Format {
        Format {
            rate,
            channels,
            sample: SampleFormat::F32,
        }
    }

    fn current(feed: &Feed) -> Option<Arc<Bridge>> {
        feed.current()
    }

    #[test]
    fn the_bridge_is_made_from_the_calls_formats_once_both_sides_called() {
        let feed = Feed::new(1.0);
        feed.on_input(&[0.5; 960], format(48000, 2));
        assert!(current(&feed).is_none(), "no output format yet");
        let mut out = [1.0; 64];
        feed.fill(&mut out, format(44100, 1));
        assert!(out.iter().all(|s| *s == 0.0), "silence before priming");
        let bridge = current(&feed).expect("both formats known");
        assert_eq!(
            bridge.input(),
            resample::Format {
                rate: 48000,
                channels: 2
            }
        );
        assert_eq!(
            bridge.output(),
            resample::Format {
                rate: 44100,
                channels: 1
            }
        );
        // Same formats keep the bridge; another output format makes a new one.
        feed.on_input(&[0.5; 960], format(48000, 2));
        assert!(Arc::ptr_eq(&bridge, &current(&feed).unwrap()));
        feed.fill(&mut out, format(48000, 1));
        assert!(!Arc::ptr_eq(&bridge, &current(&feed).unwrap()));
    }

    #[test]
    fn an_input_reopen_starts_an_empty_bridge_with_the_new_format() {
        let feed = Feed::new(1.0);
        feed.on_input(&[0.5; 960], format(48000, 2));
        feed.fill(&mut [0.0; 64], format(48000, 2));
        assert!(current(&feed).is_some());
        feed.restart_input();
        assert!(current(&feed).is_none());
        feed.fill(&mut [0.0; 64], format(48000, 2));
        assert!(current(&feed).is_none(), "waits for the new input's first call");
        feed.on_input(&[0.5; 32], format(32000, 1));
        assert_eq!(
            current(&feed).unwrap().input(),
            resample::Format {
                rate: 32000,
                channels: 1
            }
        );
    }

    #[test]
    fn a_queued_session_takes_the_reopens_and_the_gain_asked_meanwhile() {
        let mut queued = Queued {
            request: SessionRequest {
                number: 2,
                input: Some("card".into()),
                output: None,
                input_exclusive: true,
                output_exclusive: false,
            },
            gain: 1.0,
        };
        queued.apply(&Command::ReopenInput {
            input: "mic".into(),
            exclusive: false,
        });
        queued.apply(&Command::ReopenOutput {
            output: Some("headset".into()),
            exclusive: true,
        });
        queued.apply(&Command::SetGain(0.25));
        assert_eq!(
            queued,
            Queued {
                request: SessionRequest {
                    number: 2,
                    input: Some("mic".into()),
                    output: Some("headset".into()),
                    input_exclusive: false,
                    output_exclusive: true,
                },
                gain: 0.25,
            }
        );
    }

    #[test]
    fn older_sessions_hold_a_new_one_back_at_most_the_close_wait() {
        assert_eq!(held_until(std::iter::empty()), None);
        let start = Instant::now();
        let later = start + Duration::from_millis(300);
        assert_eq!(held_until([start, later].into_iter()), Some(later + CLOSE_WAIT));
    }

    #[test]
    fn closed_wakes_a_waiter_and_a_missing_close_times_out() {
        let closed = Arc::new(Closed::default());
        assert!(!closed.wait_until(Instant::now() + Duration::from_millis(20)));
        let setter = Arc::clone(&closed);
        let thread = std::thread::spawn(move || setter.set());
        assert!(closed.wait_until(Instant::now() + CLOSE_WAIT));
        thread.join().unwrap();
    }
}
