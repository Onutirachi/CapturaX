//! The video thread (research R3, R4), ported from the stack spike: it owns the GPU and presents as
//! soon as a video frame or a UI image arrives, compositing the UI over the last video frame. The
//! source hands it frames directly and the main thread hands it the UI, so the video never waits
//! for the window's event loop, which Windows holds while the window is moved or resized.

mod gpu;

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

/// What waits for the video thread; only the newest of each is kept.
#[derive(Default)]
struct State {
    frame: Option<Frame>,
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

    /// From the source: replaces a frame not yet presented.
    pub fn send_frame(&self, frame: Frame) {
        let mut state = self.lock();
        state.received += 1;
        if state.frame.replace(frame).is_some() {
            state.replaced += 1;
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
    /// the old window) goes; the newest video frame stays.
    fn begin(&self) {
        let mut state = self.lock();
        let frame = state.frame.take();
        *state = State {
            frame,
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

/// How long the video may go without a frame and still count as flowing: while it flows, a UI or
/// scene change waits for the next video frame instead of being presented on its own (research R3).
const FLOWING: Duration = Duration::from_millis(50);

/// When a UI or scene change with no video frame may be presented on its own: at once (`None`)
/// unless the video is flowing, else when it stops flowing.
fn ui_deadline(last_frame: Option<Instant>, now: Instant) -> Option<Instant> {
    last_frame.map(|t| t + FLOWING).filter(|deadline| *deadline > now)
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
    loop {
        let (frame, ui, present) = {
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
                    ui_deadline(last_frame, now)
                } else {
                    None
                };
                if state.hold.is_some_and(|hold| now >= hold.until) {
                    log::warn!("no new size within {} ms of a window change", HOLD_LIMIT.as_millis());
                    state.hold = None;
                }
                let held = state.hidden || state.hold.is_some();
                let ready = state.frame.is_some() || (state.changed && deadline.is_none());
                if state.stop || (!held && ready) {
                    break ready;
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
            (frame, state.ui.take(), present)
        };
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
        }
    }
}

/// `--stats` output goes to stdout and the log (contracts/cli.md).
fn print_stats(line: &str) {
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

    #[test]
    fn a_new_video_thread_keeps_only_the_newest_frame() {
        let inbox = Inbox::default();
        let plane = |len: usize| capture::Plane {
            bytes: vec![0; len].into(),
            stride: 2,
        };
        inbox.send_frame(Frame::new(
            2,
            2,
            capture::PixelFormat::Nv12,
            vec![plane(4), plane(2)],
            capture::Matrix::Bt709,
            capture::Range::Limited,
            Instant::now(),
            Instant::now(),
            1,
        ));
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
        assert!(!state.stop && !state.hidden && !state.changed);
        assert!(state.hold.is_none() && state.scene.is_none() && state.ui.is_none());
    }

    #[test]
    fn ui_changes_wait_only_while_the_video_flows() {
        let now = Instant::now();
        // No video yet: at once.
        assert_eq!(ui_deadline(None, now), None);
        // A frame 10 ms ago: wait for the next one, at most until the video counts as stopped.
        let last = now - Duration::from_millis(10);
        assert_eq!(ui_deadline(Some(last), now), Some(last + FLOWING));
        // No frame for longer than that: at once.
        assert_eq!(ui_deadline(Some(now - FLOWING), now), None);
        assert_eq!(ui_deadline(Some(now - Duration::from_millis(200)), now), None);
    }
}
