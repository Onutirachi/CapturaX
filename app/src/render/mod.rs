//! The video thread (research R3, R4), ported from the stack spike: it owns the GPU and presents as
//! soon as a video frame or a UI image arrives, compositing the UI over the last video frame. The
//! source hands it frames directly and the main thread hands it the UI, so the video never waits
//! for the window's event loop, which Windows holds while the window is moved or resized.

mod gpu;

use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use winit::window::Window;

use crate::cli::{Backend, PresentMode};
use crate::present_log::Stage;
use crate::stats::{Stats, Summary};
use crate::ui::UiImage;

pub use capture::Frame;
pub use gpu::RendererError;

/// What the video thread draws besides the layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scene {
    /// Where the video is letterboxed, in physical pixels: x, y, width, height, corner radius.
    pub video: [f32; 5],
    /// The ground around the video (the theme's deepest tone, or black): RGB from 0 to 1.
    pub backdrop: [f64; 3],
    /// The watermark's color (RGB from 0 to 1) and opacity; opacity 0 draws none (research R18).
    pub watermark: [f32; 4],
    /// The video's shadow: the display scale it is drawn at and its strength, 0 for none
    /// (research R18).
    pub shadow: [f32; 2],
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
    /// Follow the monitor: a present at every refresh while a video shows (research R9).
    follow: bool,
    /// The window is on screen (shown, not minimized), as the main thread last told. Follow the
    /// monitor waits for it: a Fifo present the compositor does not take makes the next acquire
    /// wait about a second.
    shown: bool,
    /// An acquire with Follow the monitor waited too long (see [`STALL`]); Follow waits for the
    /// main thread to tell the window is shown again.
    stalled: bool,
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

    /// From the main thread: the Presentation setting (FR-022, research R9). The video thread
    /// reconfigures the surface on its next turn; the session is not touched.
    pub fn set_presentation(&self, presentation: settings::Presentation) {
        self.lock().follow = presentation == settings::Presentation::FollowMonitor;
        self.wake.notify_one();
    }

    /// From the main thread: whether the window is on screen (shown and not minimized). Telling
    /// it is shown also ends a stall (see [`STALL`]).
    pub fn set_shown(&self, shown: bool) {
        let mut state = self.lock();
        state.shown = shown;
        if shown {
            state.stalled = false;
        }
        drop(state);
        self.wake.notify_one();
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
    /// the old window) goes; the newest video frame, the source's pace, the refresh and the
    /// Presentation setting stay.
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
            follow: state.follow,
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

/// Called by the video thread with each second's summary while frames are presented, and once
/// after they stop (the bubbles, FR-016).
pub type OnSecond = Box<dyn Fn(Summary) + Send>;

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
    /// (SC-004); with `stats` the thread prints contracts/stats.md. `on_second` gets each second's
    /// summary.
    pub fn start(
        window: Arc<Window>,
        inbox: Arc<Inbox>,
        presentation: Presentation,
        process_start: Option<Instant>,
        stats: bool,
        on_second: OnSecond,
    ) -> Result<Self, RendererError> {
        let gpu = gpu::Gpu::new(window, presentation)?;
        inbox.begin();
        let thread_inbox = inbox.clone();
        let handle = std::thread::Builder::new()
            .name("video".into())
            .spawn(move || run(gpu, &thread_inbox, process_start, stats, &on_second))
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

/// How long the video fades out when its source closes and in on a new source's first frame
/// (FR-013, research R17).
const FADE: Duration = Duration::from_millis(150);

/// The video's opacity going from `from` to `to` over [`FADE`] from `start`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fade {
    from: f32,
    to: f32,
    start: Instant,
}

impl Fade {
    fn new(from: f32, to: f32, start: Instant) -> Self {
        Self { from, to, start }
    }

    fn opacity(&self, now: Instant) -> f32 {
        let t = now.saturating_duration_since(self.start).as_secs_f32() / FADE.as_secs_f32();
        self.from + (self.to - self.from) * t.clamp(0.0, 1.0)
    }

    fn done(&self, now: Instant) -> bool {
        now >= self.start + FADE
    }
}

/// Between two presents of a fade with no video frame: the monitor's refresh, 60 Hz until known.
fn fade_step(refresh: Duration) -> Duration {
    if refresh.is_zero() {
        Duration::from_millis(16)
    } else {
        refresh.max(Duration::from_millis(4))
    }
}

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

    /// Follow the monitor applies now: chosen, the window on screen and no stall.
    fn follows(&self) -> bool {
        self.follow && self.shown && !self.stalled
    }
}

/// A Follow the monitor acquire longer than this means the compositor is not taking the window's
/// presents (it is not on screen yet, or minimized where the main thread cannot tell).
const STALL: Duration = Duration::from_millis(250);

fn run(mut gpu: gpu::Gpu, inbox: &Inbox, process_start: Option<Instant>, lines: bool, on_second: &OnSecond) {
    let mut follow = inbox.lock().follows();
    gpu.set_follow(follow);
    if lines {
        print_stats(&gpu.header());
    }
    let mut stats = Stats::new(Instant::now(), lines);
    // The last summary handed over: empty seconds are handed over once.
    let mut last_summary = Summary::EMPTY;
    let mut scene: Option<Scene> = None;
    let mut first_present = process_start;
    // When the last video frame was taken from the inbox.
    let mut last_frame: Option<Instant> = None;
    // Dropped after the lock on return (declared before it).
    let mut performance: Option<Performance> = None;
    // The video's fade, the session of the frame on the GPU (a frame of another one fades in) and
    // the last present (fade steps with no video frame come one refresh apart).
    let mut fade: Option<Fade> = None;
    let mut video_session: Option<u64> = None;
    let mut last_present: Option<Instant> = None;
    // A language switch's UI image was uploaded then: its present ends the switch's last stage.
    let mut language_upload: Option<Instant> = None;
    loop {
        // Follow the monitor (research R9): the surface follows the setting, and while a video
        // shows the loop is paced by the swap chain: acquire first (it blocks until about the next
        // refresh), then take the newest frame or repeat the last one. Taking the frame before the
        // acquire would make it up to one refresh older.
        // While the window is not on screen the surface goes back to the start mode.
        let (wanted, can_pace, refresh) = {
            let state = inbox.lock();
            (
                state.follows(),
                !state.hidden && state.hold.is_none() && !state.stop,
                state.refresh,
            )
        };
        if wanted != follow {
            follow = wanted;
            gpu.set_follow(follow);
            if lines {
                print_stats(&gpu.header());
            }
        }
        let acquired = if follow && can_pace && gpu.has_video() {
            // A surface that does not block (an occluded window) must not spin the loop.
            if let Some(floor) = last_present.map(|at| at + fade_step(refresh) / 2) {
                std::thread::sleep(floor.saturating_duration_since(Instant::now()));
            }
            let start = Instant::now();
            let acquired = gpu.acquire();
            let waited = start.elapsed();
            if waited > STALL {
                log::warn!(
                    "follow the monitor: an acquire waited {} ms, back to the start mode until the window is shown",
                    waited.as_millis()
                );
                inbox.lock().stalled = true;
                // Not presented: another present the compositor does not take would stall again.
                None
            } else {
                acquired
            }
        } else {
            None
        };
        let paced = acquired.is_some();
        // Handed over outside the lock.
        let mut second: Option<Summary> = None;
        let mut lines_due: Vec<String> = Vec::new();
        let (frame, ui, present, wants_performance) = {
            let mut state = inbox.lock();
            let present = loop {
                let now = Instant::now();
                stats.received(std::mem::take(&mut state.received));
                stats.replaced(std::mem::take(&mut state.replaced));
                stats.window_moved(std::mem::take(&mut state.moves));
                if let Some(report) = stats.report(now) {
                    if let Some(line) = &report.line {
                        lines_due.push(line.clone());
                    }
                    if report.summary != Summary::EMPTY || last_summary != Summary::EMPTY {
                        last_summary = report.summary;
                        second = Some(report.summary);
                    }
                }
                // A fade step is due like a UI change: on the next video frame when one is near.
                let step = fade_step(state.refresh);
                let next_step = fade.and(last_present.map(|at| at + step));
                let fade_due = fade.is_some() && next_step.is_none_or(|at| now >= at);
                let pending = state.changed || fade_due || state.clear_video;
                let deadline = if pending {
                    ui_deadline(last_frame, state.pace.period(), state.refresh, now)
                } else {
                    None
                };
                if state.hold.is_some_and(|hold| now >= hold.until) {
                    log::warn!("no new size within {} ms of a window change", HOLD_LIMIT.as_millis());
                    state.hold = None;
                }
                let held = state.hidden || state.hold.is_some();
                // An image is acquired: it is presented with or without a new frame.
                if paced && !held {
                    break true;
                }
                let ready = state.frame.is_some() || (pending && deadline.is_none());
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
                    stats.next_report(),
                    deadline.filter(|_| !held),
                    state.hold.map(|hold| hold.until),
                    next_step.filter(|_| !fade_due && !held),
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
                drop(state);
                lines_due.iter().for_each(|line| print_stats(line));
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
            // The source closed: its last frame fades out, then goes (no old frame under the next
            // state or device, FR-013); no video frame is expected any more.
            if std::mem::take(&mut state.clear_video) {
                last_frame = None;
                video_session = None;
                if gpu.has_video() {
                    let now = Instant::now();
                    let from = fade.map_or(1.0, |f| f.opacity(now));
                    fade = Some(Fade::new(from, 0.0, now));
                }
            }
            (frame, state.ui.take(), present, state.wants_performance())
        };
        lines_due.iter().for_each(|line| print_stats(line));
        if let Some(summary) = second {
            on_second(summary);
        }
        if !wants_performance {
            performance = None;
        }
        if let Some(ui) = ui {
            let start = Instant::now();
            gpu.set_ui(&ui);
            stats.ui_image(ui.draw_ms);
            if ui.language {
                let ms = start.elapsed().as_secs_f64() * 1e3;
                crate::present_log::stage(Stage::Upload, ms, lines);
                language_upload = Some(Instant::now());
            }
        }
        // Nothing to draw until the main thread has handed over the first scene.
        let Some(scene) = scene.filter(|_| present) else {
            continue;
        };
        let now = Instant::now();
        // The same source's frame shown again (Follow the monitor's repeats, the `rep` count).
        let repeat = paced && frame.is_none() && video_session.is_some();
        // A new source's first frame fades in, also over the old one fading out.
        if let Some(frame) = &frame
            && video_session != Some(frame.session)
        {
            video_session = Some(frame.session);
            fade = Some(Fade::new(0.0, 1.0, now));
        }
        let times = frame.map(|frame| {
            gpu.set_video(&frame);
            (frame.captured, frame.ready)
        });
        let opacity = fade.map_or(1.0, |f| f.opacity(now));
        if let Some(done) = fade.filter(|f| f.done(now)) {
            fade = None;
            if done.to == 0.0 {
                gpu.clear_video();
            }
        }
        // Also when the surface was not ready: the next fade step waits one refresh either way.
        last_present = Some(now);
        if gpu.render(acquired, &scene, opacity) {
            if repeat {
                stats.repeated();
            }
            if let Some(upload) = language_upload.take() {
                let ms = upload.elapsed().as_secs_f64() * 1e3;
                crate::present_log::stage(Stage::Present, ms, lines);
            }
            if let Some(process_start) = first_present.take() {
                log::info!(
                    "first present {:.0} ms after start",
                    process_start.elapsed().as_secs_f64() * 1e3
                );
            }
            if let Some((captured, ready)) = times {
                stats.presented(captured, ready, Instant::now());
            }
            // After the first frame is on screen, so loading the GPU library does not delay it.
            if times.is_some() && wants_performance && performance.is_none() {
                performance = Some(Performance::acquire(gpu.pci_bus_id()));
            }
        }
    }
}

/// What the stats writer thread is handed.
enum StatsOut {
    Line(String),
    /// Answered once every line handed over before it is written.
    Flush(mpsc::SyncSender<()>),
}

/// The stats writer, started on the first line; `None` when its thread could not start.
static STATS_WRITER: OnceLock<Option<Sender<StatsOut>>> = OnceLock::new();

/// `--stats` output goes to stdout and the log (contracts/cli.md), written by a thread of its own:
/// a slow disk must not hold the caller, the video thread above all (measurements/stalls.md).
pub fn print_stats(line: &str) {
    match stats_writer() {
        Some(writer) if writer.send(StatsOut::Line(line.to_owned())).is_ok() => {}
        _ => write_stats(line),
    }
}

/// Waits up to `limit` for the stats lines handed over so far to be written (before exiting).
pub fn flush_stats(limit: Duration) {
    if let Some(Some(writer)) = STATS_WRITER.get() {
        let (done, wait) = mpsc::sync_channel(1);
        if writer.send(StatsOut::Flush(done)).is_ok() {
            let _ = wait.recv_timeout(limit);
        }
    }
}

fn stats_writer() -> Option<&'static Sender<StatsOut>> {
    STATS_WRITER
        .get_or_init(|| {
            let (send, receive) = mpsc::channel();
            let spawned = std::thread::Builder::new().name("stats".into()).spawn(move || {
                for out in receive {
                    match out {
                        StatsOut::Line(line) => write_stats(&line),
                        StatsOut::Flush(done) => {
                            let _ = std::io::Write::flush(&mut std::io::stdout());
                            let _ = done.send(());
                        }
                    }
                }
            });
            match spawned {
                Ok(_) => Some(send),
                Err(err) => {
                    log::warn!("stats writer: {err}; lines written by the caller");
                    None
                }
            }
        })
        .as_ref()
}

fn write_stats(line: &str) {
    println!("{line}");
    log::info!("{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_goes_from_its_start_to_its_end_in_150_ms() {
        let start = Instant::now();
        let fade = Fade::new(0.0, 1.0, start);
        assert_eq!(fade.opacity(start), 0.0);
        assert!((fade.opacity(start + FADE / 2) - 0.5).abs() < 1e-3);
        assert_eq!(fade.opacity(start + FADE), 1.0);
        assert_eq!(fade.opacity(start + FADE * 2), 1.0, "stays at its end");
        assert!(!fade.done(start + FADE / 2) && fade.done(start + FADE));
        // A fade out from where a fade in stopped.
        let out = Fade::new(0.4, 0.0, start);
        assert!((out.opacity(start + FADE / 2) - 0.2).abs() < 1e-3);
        assert_eq!(fade_step(Duration::ZERO), Duration::from_millis(16));
        assert_eq!(fade_step(Duration::from_millis(7)), Duration::from_millis(7));
    }

    #[test]
    fn a_hold_ends_with_a_scene_of_another_size() {
        let scene = |size| Scene {
            video: [0.0; 5],
            backdrop: [0.0; 3],
            watermark: [0.0; 4],
            shadow: [1.0, 0.0],
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
                watermark: [0.0; 4],
                shadow: [1.0, 0.0],
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
