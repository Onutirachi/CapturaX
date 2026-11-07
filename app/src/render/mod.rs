//! The video thread (research R3, R4), ported from the stack spike: it owns the GPU and presents as
//! soon as a video frame or a UI image arrives, compositing the UI over the last video frame. The
//! source hands it frames directly and the main thread hands it the UI, so the video never waits
//! for the window's event loop, which Windows holds while the window is moved or resized.

mod gpu;

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use winit::window::Window;

use crate::cli::{Backend, PresentMode};
use crate::stats::Stats;
use crate::ui::UiImage;

pub use capture::Frame;
pub use gpu::RendererError;

/// What the video thread draws besides the layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scene {
    /// Where the video is letterboxed, in physical pixels: x, y, width, height, corner radius.
    pub video: [f32; 5],
    /// The ground around the video (the theme's deepest tone): RGB from 0 to 1.
    pub backdrop: [f64; 3],
    /// The surface size in physical pixels.
    pub size: (u32, u32),
}

/// The session the video thread takes frames from (see [`Inbox::set_session`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feed {
    pub session: u64,
    /// The open mode's frame rate.
    pub fps: f64,
    /// Ask the GPU for full clocks while the session's frames are presented (FR-012, research R7):
    /// a device, or the test video when `--stats` compares runs.
    pub gpu_performance: bool,
}

/// What waits for the video thread; only the newest of each is kept.
#[derive(Default)]
struct State {
    frame: Option<Frame>,
    /// The session whose frames are taken; others (a closed session's last frames) are dropped.
    session: Option<u64>,
    /// The session asks for full GPU clocks while it is presented.
    gpu_performance: bool,
    /// When the session's frames come (research R16).
    pace: Pace,
    /// The refresh interval of the monitor the window is on; zero until the main thread tells.
    refresh: Duration,
    /// The video layer goes before the next present (the source closed).
    clear_video: bool,
    ui: Option<UiImage>,
    scene: Option<Scene>,
    /// The UI or the scene changed since the video thread last looked.
    changed: bool,
    /// Presenting is off while the window is hidden.
    hidden: bool,
    /// Presenting waits for a scene of another size (see [`Inbox::hold_for_resize`]).
    hold: Option<Hold>,
    stop: bool,
    /// Counted for the stats since the video thread last looked.
    received: u32,
    replaced: u32,
    moves: u32,
}

/// The source's frame period: the median of the recent arrival intervals, the open mode's rate
/// before there are any (research R16).
#[derive(Debug, Default, Clone)]
struct Pace {
    /// From the open mode's frame rate.
    nominal: Option<Duration>,
    /// The latest arrival intervals, at most [`PACE_INTERVALS`].
    intervals: VecDeque<Duration>,
    last_arrival: Option<Instant>,
}

/// How many arrival intervals the median is taken over: half a second at 30 fps.
const PACE_INTERVALS: usize = 15;

impl Pace {
    fn new(fps: f64) -> Self {
        Self {
            nominal: (fps > 0.0).then(|| Duration::from_secs_f64(1.0 / fps)),
            ..Self::default()
        }
    }

    fn arrived(&mut self, at: Instant) {
        if let Some(last) = self.last_arrival.replace(at) {
            if self.intervals.len() == PACE_INTERVALS {
                self.intervals.pop_front();
            }
            self.intervals.push_back(at.saturating_duration_since(last));
        }
    }

    fn period(&self) -> Option<Duration> {
        if self.intervals.is_empty() {
            return self.nominal;
        }
        let mut sorted: Vec<Duration> = self.intervals.iter().copied().collect();
        sorted.sort_unstable();
        Some(sorted[sorted.len() / 2])
    }
}

/// No present until the surface size changes from `size`, or `until` passes.
#[derive(Debug, Clone, Copy)]
struct Hold {
    size: (u32, u32),
    until: Instant,
}

/// The longest a [`Hold`] lasts, for a size change that never comes.
const HOLD_LIMIT: Duration = Duration::from_millis(100);

/// Hands frames and UI images to the video thread and wakes it.
#[derive(Default)]
pub struct Inbox {
    state: Mutex<State>,
    wake: Condvar,
}

impl Inbox {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// From the source: replaces a frame not yet presented. A frame of another session than the
    /// current one is dropped.
    pub fn send_frame(&self, frame: Frame) {
        let mut state = self.lock();
        if state.session != Some(frame.session) {
            return;
        }
        state.received += 1;
        state.pace.arrived(Instant::now());
        if state.frame.replace(frame).is_some() {
            state.replaced += 1;
        }
        drop(state);
        self.wake.notify_one();
    }

    /// From the main thread: the session whose frames are taken from now on (`None`: none). A
    /// waiting frame of another session goes.
    pub fn set_session(&self, feed: Option<Feed>) {
        let mut state = self.lock();
        state.pace = feed.map(|feed| Pace::new(feed.fps)).unwrap_or_default();
        state.gpu_performance = feed.is_some_and(|feed| feed.gpu_performance);
        let session = feed.map(|feed| feed.session);
        state.session = session;
        if state.frame.as_ref().is_some_and(|f| Some(f.session) != session) {
            state.frame = None;
        }
        drop(state);
        // A held GPU request goes when the session ends.
        self.wake.notify_one();
    }

    /// From the main thread: the source closed, so its last frame is not shown again (no old
    /// frame under the next state or the next device, FR-013).
    pub fn clear_video(&self) {
        let mut state = self.lock();
        state.frame = None;
        state.clear_video = true;
        state.changed = true;
        drop(state);
        self.wake.notify_one();
    }

    /// From the main thread: the refresh interval of the monitor the window is on.
    pub fn set_refresh(&self, refresh: Duration) {
        self.lock().refresh = refresh;
    }

    /// From the main thread: the window moved (for the stats).
    pub fn window_moved(&self) {
        self.lock().moves += 1;
    }

    /// From the main thread: the UI image when it was redrawn, and the scene.
    pub fn send_ui(&self, ui: Option<UiImage>, scene: Scene) {
        let mut state = self.lock();
        if ui.is_some() {
            state.ui = ui;
        }
        if state.hold.is_some_and(|hold| hold.size != scene.size) {
            state.hold = None;
        }
        state.scene = Some(scene);
        state.changed = true;
        drop(state);
        self.wake.notify_one();
    }

    /// For a new video thread: what the last one left (the stop, a hold, the UI image and scene of
    /// the old window) goes; the newest video frame, the source's pace and the refresh stay.
    fn begin(&self) {
        let mut state = self.lock();
        let frame = state.frame.take();
        let pace = std::mem::take(&mut state.pace);
        *state = State {
            frame,
            session: state.session,
            gpu_performance: state.gpu_performance,
            pace,
            refresh: state.refresh,
            ..State::default()
        };
    }

    /// From the main thread, just before asking the system for a new window size: no present
    /// until the UI hands over a scene of another size, at most [`HOLD_LIMIT`]. On Wayland the
    /// first commit after the compositor's configure tells it the window's new size; a frame of
    /// the old size there made KWin put the window off its place when leaving fullscreen
    /// (research R5).
    pub fn hold_for_resize(&self) {
        let mut state = self.lock();
        if let Some(scene) = state.scene {
            state.hold = Some(Hold {
                size: scene.size,
                until: Instant::now() + HOLD_LIMIT,
            });
        }
    }
}

/// Presentation forced from the command line (research R4); none = the platform's order.
#[derive(Debug, Clone, Copy, Default)]
pub struct Presentation {
    pub backend: Option<Backend>,
    pub present: Option<PresentMode>,
}

/// The running video thread; stopped before the window is destroyed.
pub struct VideoThread {
    inbox: Arc<Inbox>,
    handle: Option<JoinHandle<()>>,
}

impl VideoThread {
    /// Sets up the GPU for `window` and starts the thread on `inbox`, which outlives it: a window
    /// made again after hiding on Wayland gets a new thread on the same inbox, so the source keeps
    /// sending (research R11). With `process_start` the time to the first present is logged
    /// (SC-004); with `stats` the thread prints contracts/stats.md.
    pub fn start(
        window: Arc<Window>,
        inbox: Arc<Inbox>,
        presentation: Presentation,
        process_start: Option<Instant>,
        stats: bool,
    ) -> Result<Self, RendererError> {
        let gpu = gpu::Gpu::new(window, presentation)?;
        inbox.begin();
        let thread_inbox = inbox.clone();
        let handle = std::thread::Builder::new()
            .name("video".into())
            .spawn(move || run(gpu, &thread_inbox, process_start, stats))
            .map_err(|e| RendererError(format!("video thread: {e}")))?;
        Ok(Self {
            inbox,
            handle: Some(handle),
        })
    }

    pub fn inbox(&self) -> &Arc<Inbox> {
        &self.inbox
    }

    /// Stops or resumes presenting (the window is hidden in the tray).
    pub fn set_visible(&self, visible: bool) {
        let mut state = self.inbox.lock();
        state.hidden = !visible;
        state.changed = true;
        drop(state);
        self.inbox.wake.notify_one();
    }

    /// Stops the thread and waits for it; the surface is released before the window.
    pub fn stop(&mut self) {
        self.inbox.lock().stop = true;
        self.inbox.wake.notify_one();
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            log::error!("the video thread panicked");
        }
    }
}

impl Drop for VideoThread {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Without a frame period, how long the video may go without a frame and still count as flowing
/// (research R3).
const FLOWING: Duration = Duration::from_millis(50);

/// Added to the monitor's refresh interval for the time a UI present may hold the swap chain:
/// frame arrival jitter and the present's own cost (research R16).
const PRESENT_MARGIN: Duration = Duration::from_millis(2);

/// When a UI or scene change with no video frame may be presented on its own (`None`: at once).
///
/// A UI present can make the next video frame wait up to one refresh, so the change rides on the
/// next video frame (research R3) only when that frame is expected within one refresh plus
/// [`PRESENT_MARGIN`]; else it is presented between frames (research R16). The expected frame is
/// `last_frame + period`; one later than the same window counts as late and the change goes on
/// its own. Without a period the video counts as flowing for [`FLOWING`] after a frame.
fn ui_deadline(
    last_frame: Option<Instant>,
    period: Option<Duration>,
    refresh: Duration,
    now: Instant,
) -> Option<Instant> {
    let last = last_frame?;
    let deadline = match period {
        Some(period) => {
            let expected = last + period;
            let window = refresh + PRESENT_MARGIN;
            if expected > now + window {
                return None;
            }
            expected + window
        }
        None => last + FLOWING,
    };
    Some(deadline).filter(|deadline| *deadline > now)
}

/// The GPU performance request (FR-012, research R7), held while the frames of a session that asks
/// for it are presented; acquiring and releasing are logged.
struct Performance {
    _guard: platform::PerformanceGuard,
}

impl Performance {
    fn acquire(pci_bus_id: Option<&str>) -> Self {
        log::info!(
            "gpu performance requested (adapter {})",
            pci_bus_id.unwrap_or("unknown")
        );
        Self {
            _guard: platform::gpu_performance(pci_bus_id),
        }
    }
}

impl Drop for Performance {
    fn drop(&mut self) {
        log::info!("gpu performance released");
    }
}

impl State {
    /// The GPU request is wanted: a session that asks for it is open and the window shows.
    fn wants_performance(&self) -> bool {
        self.gpu_performance && self.session.is_some() && !self.hidden
    }
}

fn run(mut gpu: gpu::Gpu, inbox: &Inbox, process_start: Option<Instant>, stats: bool) {
    let mut stats = stats.then(|| {
        print_stats(&gpu.header());
        Stats::new(Instant::now())
    });
    let mut scene: Option<Scene> = None;
    let mut first_present = process_start;
    // When the last video frame was taken from the inbox.
    let mut last_frame: Option<Instant> = None;
    // Dropped after the lock on return (declared before it).
    let mut performance: Option<Performance> = None;
    loop {
        let (frame, ui, present, wants_performance) = {
            let mut state = inbox.lock();
            let present = loop {
                let now = Instant::now();
                if let Some(stats) = &mut stats {
                    stats.received(std::mem::take(&mut state.received));
                    stats.replaced(std::mem::take(&mut state.replaced));
                    stats.window_moved(std::mem::take(&mut state.moves));
                    if let Some(line) = stats.report(now) {
                        print_stats(&line);
                    }
                }
                let deadline = if state.changed {
                    ui_deadline(last_frame, state.pace.period(), state.refresh, now)
                } else {
                    None
                };
                if state.hold.is_some_and(|hold| now >= hold.until) {
                    log::warn!("no new size within {} ms of a window change", HOLD_LIMIT.as_millis());
                    state.hold = None;
                }
                let held = state.hidden || state.hold.is_some();
                let ready = state.frame.is_some() || (state.changed && deadline.is_none());
                // A request no longer wanted is released outside the lock.
                let release = performance.is_some() && !state.wants_performance();
                if state.stop || (!held && ready) || release {
                    break !held && ready;
                }
                // A UI image is uploaded as soon as it arrives, so the next video frame's way to
                // the screen does not include it (SC-001).
                if !state.hidden && state.ui.is_some() {
                    break false;
                }
                // Wake for the next stats line, a waiting change and the hold's end, even when
                // nothing arrives.
                let wake_at = [
                    stats.as_ref().map(Stats::next_report),
                    deadline.filter(|_| !held),
                    state.hold.map(|hold| hold.until),
                ]
                .into_iter()
                .flatten()
                .min();
                state = match wake_at {
                    Some(at) => {
                        let timeout = at.saturating_duration_since(now).max(Duration::from_millis(1));
                        inbox
                            .wake
                            .wait_timeout(state, timeout)
                            .unwrap_or_else(|e| e.into_inner())
                            .0
                    }
                    None => inbox.wake.wait(state).unwrap_or_else(|e| e.into_inner()),
                };
            };
            if state.stop {
                return;
            }
            let frame = if present {
                state.changed = false;
                scene = state.scene.or(scene);
                state.frame.take()
            } else {
                None
            };
            if frame.is_some() {
                last_frame = Some(Instant::now());
            }
            if std::mem::take(&mut state.clear_video) {
                gpu.clear_video();
            }
            (frame, state.ui.take(), present, state.wants_performance())
        };
        if !wants_performance {
            performance = None;
        }
        if let Some(ui) = ui {
            gpu.set_ui(&ui);
            if let Some(stats) = &mut stats {
                stats.ui_image(ui.draw_ms);
            }
        }
        // Nothing to draw until the main thread has handed over the first scene.
        let Some(scene) = scene.filter(|_| present) else {
            continue;
        };
        gpu.resize(scene.size.0, scene.size.1);
        let ready = frame.map(|frame| {
            gpu.set_video(&frame);
            frame.ready
        });
        if gpu.render(&scene) {
            if let Some(process_start) = first_present.take() {
                log::info!(
                    "first present {:.0} ms after start",
                    process_start.elapsed().as_secs_f64() * 1e3
                );
            }
            if let (Some(stats), Some(ready)) = (&mut stats, ready) {
                stats.presented(ready, Instant::now());
            }
            // After the first frame is on screen, so loading the GPU library does not delay it.
            if ready.is_some() && wants_performance && performance.is_none() {
                performance = Some(Performance::acquire(gpu.pci_bus_id()));
            }
        }
    }
}

/// `--stats` output goes to stdout and the log (contracts/cli.md).
pub fn print_stats(line: &str) {
    println!("{line}");
    log::info!("{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_ends_with_a_scene_of_another_size() {
        let scene = |size| Scene {
            video: [0.0; 5],
            backdrop: [0.0; 3],
            size,
        };
        let inbox = Inbox::default();
        // Nothing presented yet: nothing to hold.
        inbox.hold_for_resize();
        assert!(inbox.lock().hold.is_none());
        inbox.send_ui(None, scene((1920, 1080)));
        inbox.hold_for_resize();
        let hold = inbox.lock().hold.expect("held");
        assert_eq!(hold.size, (1920, 1080));
        assert!(hold.until > Instant::now() && hold.until <= Instant::now() + HOLD_LIMIT);
        // A UI change at the old size keeps it; the new size ends it.
        inbox.send_ui(None, scene((1920, 1080)));
        assert!(inbox.lock().hold.is_some());
        inbox.send_ui(None, scene((1280, 720)));
        assert!(inbox.lock().hold.is_none());
    }

    fn feed(session: u64, fps: f64) -> Feed {
        Feed {
            session,
            fps,
            gpu_performance: true,
        }
    }

    fn frame(session: u64) -> Frame {
        let plane = |len: usize| capture::Plane {
            bytes: vec![0; len].into(),
            stride: 2,
        };
        Frame::new(
            2,
            2,
            capture::PixelFormat::Nv12,
            vec![plane(4), plane(2)],
            capture::Matrix::Bt709,
            capture::Range::Limited,
            Instant::now(),
            Instant::now(),
            session,
        )
    }

    #[test]
    fn only_the_current_session_frames_are_taken() {
        let inbox = Inbox::default();
        inbox.send_frame(frame(1));
        assert!(inbox.lock().frame.is_none());
        inbox.set_session(Some(feed(1, 30.0)));
        inbox.send_frame(frame(1));
        assert!(inbox.lock().frame.is_some());
        // A new session drops the old one's waiting frame and its late frames.
        inbox.set_session(Some(feed(2, 30.0)));
        assert!(inbox.lock().frame.is_none());
        inbox.send_frame(frame(1));
        assert!(inbox.lock().frame.is_none());
        inbox.send_frame(frame(2));
        assert_eq!(inbox.lock().frame.as_ref().map(|f| f.session), Some(2));
        inbox.set_session(None);
        assert!(inbox.lock().frame.is_none());
    }

    #[test]
    fn a_new_video_thread_keeps_only_the_newest_frame() {
        let inbox = Inbox::default();
        inbox.set_session(Some(feed(1, 30.0)));
        inbox.set_refresh(Duration::from_millis(7));
        inbox.send_frame(frame(1));
        inbox.send_ui(
            None,
            Scene {
                video: [0.0; 5],
                backdrop: [0.0; 3],
                size: (1280, 720),
            },
        );
        inbox.hold_for_resize();
        {
            let mut state = inbox.lock();
            state.hidden = true;
            state.stop = true;
        }
        inbox.begin();
        let state = inbox.lock();
        assert!(state.frame.is_some());
        assert_eq!(state.session, Some(1));
        assert!(state.pace.last_arrival.is_some());
        assert_eq!(state.refresh, Duration::from_millis(7));
        assert!(!state.stop && !state.hidden && !state.changed);
        assert!(state.hold.is_none() && state.scene.is_none() && state.ui.is_none());
    }

    fn ms(ms: f64) -> Duration {
        Duration::from_secs_f64(ms / 1e3)
    }

    /// The deadline `since` ms after a frame, for a source at `fps` on a monitor at `hz`, in ms
    /// from that frame (`None`: at once).
    fn deadline(fps: f64, hz: f64, since: f64) -> Option<f64> {
        let last = Instant::now();
        ui_deadline(Some(last), Some(ms(1e3 / fps)), ms(1e3 / hz), last + ms(since))
            .map(|d| (d - last).as_secs_f64() * 1e3)
    }

    fn near(got: Option<f64>, want: f64) -> bool {
        got.is_some_and(|got| (got - want).abs() < 0.01)
    }

    #[test]
    fn ui_changes_go_between_frames_when_a_refresh_fits() {
        // 30 fps on 60 Hz: a 33.33 ms period, a 16.67 + 2 ms window.
        assert_eq!(deadline(30.0, 60.0, 1.0), None);
        assert_eq!(deadline(30.0, 60.0, 14.0), None);
        assert!(near(deadline(30.0, 60.0, 15.0), 33.33 + 18.67));
        // 30 fps on 144 Hz: a 6.94 + 2 ms window.
        assert_eq!(deadline(30.0, 144.0, 24.0), None);
        assert!(near(deadline(30.0, 144.0, 25.0), 33.33 + 8.94));
        // 60 fps on 60 Hz: no room between frames, the change rides on the next one.
        assert!(near(deadline(60.0, 60.0, 0.0), 16.67 + 18.67));
        // 60 fps on 144 Hz: room right after a frame.
        assert_eq!(deadline(60.0, 144.0, 1.0), None);
        assert_eq!(deadline(60.0, 144.0, 7.0), None);
        assert!(near(deadline(60.0, 144.0, 8.0), 16.67 + 8.94));
        // 120 fps: never room, on either monitor.
        assert!(near(deadline(120.0, 60.0, 0.0), 8.33 + 18.67));
        assert!(near(deadline(120.0, 144.0, 0.0), 8.33 + 8.94));
        // A frame later than the window: at once.
        assert_eq!(deadline(30.0, 60.0, 52.1), None);
        assert_eq!(deadline(120.0, 144.0, 17.3), None);
    }

    #[test]
    fn ui_changes_wait_only_while_the_video_flows() {
        let now = Instant::now();
        let refresh = ms(16.67);
        // No video yet: at once.
        assert_eq!(ui_deadline(None, Some(ms(33.3)), refresh, now), None);
        assert_eq!(ui_deadline(None, None, refresh, now), None);
        // No period known: wait for the next frame, at most until the video counts as stopped.
        let last = now - Duration::from_millis(10);
        assert_eq!(ui_deadline(Some(last), None, refresh, now), Some(last + FLOWING));
        // No frame for longer than that: at once.
        assert_eq!(ui_deadline(Some(now - FLOWING), None, refresh, now), None);
        let long_ago = now - Duration::from_millis(200);
        assert_eq!(ui_deadline(Some(long_ago), None, refresh, now), None);
    }

    #[test]
    fn the_period_is_the_median_of_recent_arrivals() {
        let nominal = Some(Duration::from_secs_f64(1.0 / 30.0));
        let mut pace = Pace::new(30.0);
        assert_eq!(pace.period(), nominal);
        assert_eq!(Pace::new(0.0).period(), None);
        let mut at = Instant::now();
        // One arrival gives no interval yet.
        pace.arrived(at);
        assert_eq!(pace.period(), nominal);
        // The camera really gives 25 fps, with a late frame and a stall.
        for gap in [40, 40, 41, 39, 120, 40, 2000, 40] {
            at += Duration::from_millis(gap);
            pace.arrived(at);
        }
        assert_eq!(pace.period(), Some(Duration::from_millis(40)));
        // Only the latest intervals count.
        for _ in 0..PACE_INTERVALS {
            at += Duration::from_millis(10);
            pace.arrived(at);
        }
        assert_eq!(pace.intervals.len(), PACE_INTERVALS);
        assert_eq!(pace.period(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn the_gpu_request_follows_a_shown_session_that_asks_for_it() {
        let inbox = Inbox::default();
        assert!(!inbox.lock().wants_performance());
        inbox.set_session(Some(feed(1, 60.0)));
        assert!(inbox.lock().wants_performance());
        // A new video thread (Wayland, after hiding) keeps it.
        inbox.begin();
        assert!(inbox.lock().wants_performance());
        // Hidden in the tray: released.
        inbox.lock().hidden = true;
        assert!(!inbox.lock().wants_performance());
        inbox.lock().hidden = false;
        // The test video without `--stats` does not ask; a closed session neither.
        inbox.set_session(Some(Feed {
            gpu_performance: false,
            ..feed(0, 60.0)
        }));
        assert!(!inbox.lock().wants_performance());
        inbox.set_session(Some(feed(2, 60.0)));
        inbox.set_session(None);
        assert!(!inbox.lock().wants_performance());
    }

    #[test]
    fn a_new_session_starts_from_its_mode_rate() {
        let inbox = Inbox::default();
        inbox.set_session(Some(feed(1, 30.0)));
        inbox.send_frame(frame(1));
        inbox.send_frame(frame(1));
        assert_eq!(inbox.lock().pace.intervals.len(), 1);
        inbox.set_session(Some(feed(2, 60.0)));
        let state = inbox.lock();
        assert!(state.pace.intervals.is_empty() && state.pace.last_arrival.is_none());
        assert_eq!(state.pace.period(), Some(Duration::from_secs_f64(1.0 / 60.0)));
    }
}
