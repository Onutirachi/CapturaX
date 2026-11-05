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

pub use gpu::RendererError;

/// One video frame: RGBA, rows packed.
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    /// When the source finished it (frame ready → present, SC-001).
    pub ready: Instant,
}

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
    frame: Option<VideoFrame>,
    ui: Option<UiImage>,
    scene: Option<Scene>,
    /// The UI or the scene changed since the video thread last looked.
    changed: bool,
    /// Presenting is off while the window is hidden.
    hidden: bool,
    stop: bool,
    /// Counted for the stats since the video thread last looked.
    received: u32,
    replaced: u32,
    moves: u32,
}

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
    pub fn send_frame(&self, frame: VideoFrame) {
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
        state.scene = Some(scene);
        state.changed = true;
        drop(state);
        self.wake.notify_one();
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
    /// Sets up the GPU for `window` and starts the thread. `process_start` is used to log the time
    /// to the first present (SC-004); with `stats` the thread prints contracts/stats.md.
    pub fn start(
        window: Arc<Window>,
        presentation: Presentation,
        process_start: Instant,
        stats: bool,
    ) -> Result<Self, RendererError> {
        let gpu = gpu::Gpu::new(window, presentation)?;
        let inbox = Arc::new(Inbox::default());
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

fn run(mut gpu: gpu::Gpu, inbox: &Inbox, process_start: Instant, stats: bool) {
    let mut stats = stats.then(|| {
        print_stats(&gpu.header());
        Stats::new(Instant::now())
    });
    let mut scene: Option<Scene> = None;
    let mut first_present = true;
    loop {
        let (frame, ui) = {
            let mut state = inbox.lock();
            loop {
                if let Some(stats) = &mut stats {
                    stats.received(std::mem::take(&mut state.received));
                    stats.replaced(std::mem::take(&mut state.replaced));
                    stats.window_moved(std::mem::take(&mut state.moves));
                    if let Some(line) = stats.report(Instant::now()) {
                        print_stats(&line);
                    }
                }
                if state.stop || !(state.hidden || (state.frame.is_none() && !state.changed)) {
                    break;
                }
                state = match &stats {
                    // Wake for the next line even when nothing arrives.
                    Some(stats) => {
                        let timeout = stats
                            .next_report()
                            .saturating_duration_since(Instant::now())
                            .max(Duration::from_millis(1));
                        inbox
                            .wake
                            .wait_timeout(state, timeout)
                            .unwrap_or_else(|e| e.into_inner())
                            .0
                    }
                    None => inbox.wake.wait(state).unwrap_or_else(|e| e.into_inner()),
                };
            }
            if state.stop {
                return;
            }
            state.changed = false;
            scene = state.scene.or(scene);
            (state.frame.take(), state.ui.take())
        };
        // Nothing to draw until the main thread has handed over the first scene.
        let Some(scene) = scene else { continue };
        gpu.resize(scene.size.0, scene.size.1);
        if let Some(ui) = ui {
            gpu.set_ui(&ui);
            if let Some(stats) = &mut stats {
                stats.ui_image(ui.draw_ms);
            }
        }
        let ready = frame.map(|frame| {
            gpu.set_video(&frame);
            frame.ready
        });
        if gpu.render(&scene) {
            if first_present {
                first_present = false;
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
