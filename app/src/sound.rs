//! Runs the preview audio's commands (plan "Startup and quit", research R9): the platform's audio
//! monitor and session, and the bridge between the session's two sides. The decisions are
//! [`AudioController`]'s; this module only carries them out, and nothing here waits for an audio
//! thread except the quit, at most [`CLOSE_WAIT`].

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use ::audio::bridge::Bridge;
use ::audio::resample;
use platform::audio::{Format, Monitor, Samples, Session, SessionEvent};
use winit::event_loop::EventLoopProxy;

use crate::app::UserEvent;
use crate::audio::{AudioController, Command, Event, VideoDevice};

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
}

/// The preview's sound in the app: the controller, the monitor and the open session.
pub struct Sound {
    controller: AudioController,
    monitor: Option<Monitor>,
    open: Option<Open>,
    /// The sessions asked to close whose `Closed` has not come, by number.
    closing: Vec<(u64, Arc<Closed>)>,
    proxy: EventLoopProxy<UserEvent>,
}

impl Sound {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            controller: AudioController::new(),
            monitor: None,
            open: None,
            closing: Vec::new(),
            proxy,
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
                self.closing.retain(|(closing, _)| closing != number);
            }
            Event::Session(event @ SessionEvent::Opened { .. }) => log::info!("audio: {event:?}"),
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
    }

    fn run(&mut self, command: Command, settings: &settings::Settings) {
        match command {
            Command::Open(request) => {
                self.close();
                let number = request.number;
                let gain = ::audio::volume::gain(settings.audio.volume, settings.audio.muted);
                let feed = Arc::new(Feed::new(gain));
                let closed = Arc::new(Closed::default());
                let proxy = self.proxy.clone();
                log::info!("audio session {number}: open {} -> {:?}", request.input, request.output);
                let on_closed = Arc::clone(&closed);
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
                });
            }
            Command::ReopenInput { input, exclusive } => {
                if let Some(open) = &self.open {
                    open.feed.restart_input();
                    open.session.reopen_input(input, exclusive);
                }
            }
            Command::ReopenOutput { output, exclusive } => {
                if let Some(open) = &self.open {
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

    /// Asks the open session to close; returns at once (its `Closed` comes later).
    fn close(&mut self) {
        if let Some(mut open) = self.open.take() {
            open.session.close();
            self.closing.push((open.number, open.closed));
        }
    }

    /// Quit (research R9): the open session closes and the audio threads get at most
    /// [`CLOSE_WAIT`] to release their devices; then the monitor stops. A close that does not come
    /// is logged and the quit goes on (the process exits after the settings are saved).
    pub fn quit(&mut self) {
        self.close();
        let deadline = Instant::now() + CLOSE_WAIT;
        let started = Instant::now();
        for (number, closed) in self.closing.drain(..) {
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
        lock(&feed.formats).bridge.clone()
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
    fn closed_wakes_a_waiter_and_a_missing_close_times_out() {
        let closed = Arc::new(Closed::default());
        assert!(!closed.wait_until(Instant::now() + Duration::from_millis(20)));
        let setter = Arc::clone(&closed);
        let thread = std::thread::spawn(move || setter.set());
        assert!(closed.wait_until(Instant::now() + CLOSE_WAIT));
        thread.join().unwrap();
    }
}
