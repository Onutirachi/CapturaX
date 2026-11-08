//! The capture pipeline of one open device (research R4), ported from the stack spike's
//! `capture/src/video.rs`: `source ! capsfilter(mode) [! decoder] [! videoconvert] ! appsink`,
//! handing the device's own planes to the video thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gst::prelude::*;
use gst_video::prelude::*;

use crate::analysis::{self, RangeChoice, RangeDetector};
use crate::device::Device;
use crate::frame::{self, Frame, Matrix, PixelFormat, Plane, PlanePool, Range};
use crate::mode::{Format, Mode};

/// Why a device cannot play (research R6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceError {
    /// Another app has it open.
    InUse,
    /// The system blocks access; on Linux the node that could not be opened.
    Blocked { node: Option<String> },
    /// The device left.
    Gone,
    /// No frame within 5 s of playing.
    NoSignal,
    /// Anything else, with GStreamer's text.
    Failed(String),
}

/// Whether the system lets the app use cameras, as `platform::camera_access()` reads it; the app
/// passes it in, this crate makes no OS calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Allowed,
    Denied,
    Unknown,
}

/// What the app tells a session about the system, for mapping its errors (research R6); this
/// crate makes no OS calls and has no platform conditions.
pub struct ErrorContext {
    /// Built for Windows (`platform::IS_WINDOWS`): Media Foundation's errors are generic.
    pub windows: bool,
    /// Reads the camera privacy switch (`platform::camera_access()`) when the device fails.
    pub access: Box<dyn Fn() -> Access + Send + Sync>,
}

impl ErrorContext {
    /// No platform knowledge: the test video, and tests.
    pub fn none() -> Self {
        Self {
            windows: false,
            access: Box::new(|| Access::Unknown),
        }
    }
}

/// The GStreamer errors the mapping tells apart (research R6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ResourceBusy,
    ResourceOpenReadWrite,
    ResourceNotFound,
    ResourceRead,
    StreamFailed,
    Other,
}

impl ErrorCode {
    fn of(error: &gst::glib::Error) -> Self {
        if let Some(code) = error.kind::<gst::ResourceError>() {
            match code {
                gst::ResourceError::Busy => Self::ResourceBusy,
                gst::ResourceError::OpenReadWrite => Self::ResourceOpenReadWrite,
                gst::ResourceError::NotFound => Self::ResourceNotFound,
                gst::ResourceError::Read => Self::ResourceRead,
                _ => Self::Other,
            }
        } else if error.kind::<gst::StreamError>() == Some(gst::StreamError::Failed) {
            Self::StreamFailed
        } else {
            Self::Other
        }
    }
}

/// A `STREAM/FAILED` this soon after asking for Playing means another app holds the device on
/// Windows (research R6).
const IN_USE_WITHIN: Duration = Duration::from_secs(2);

/// No frame this long after Playing → [`DeviceError::NoSignal`].
pub const NO_SIGNAL_AFTER: Duration = Duration::from_secs(5);

/// Maps a pipeline error to why the device cannot play (research R6). `message` is GStreamer's
/// error text, `debug` its debug text; `since_playing` is the time since Playing was asked for
/// (None before); `still_listed` whether the device is listed now; `access` the camera privacy
/// switch (Windows).
pub fn map_error(
    windows: bool,
    code: ErrorCode,
    message: &str,
    debug: &str,
    since_playing: Option<Duration>,
    still_listed: bool,
    access: Access,
) -> DeviceError {
    if windows {
        if access == Access::Denied {
            DeviceError::Blocked { node: None }
        } else if !still_listed {
            DeviceError::Gone
        } else if code == ErrorCode::StreamFailed && since_playing.is_some_and(|t| t <= IN_USE_WITHIN) {
            DeviceError::InUse
        } else {
            DeviceError::Failed(message.to_string())
        }
    } else {
        match code {
            ErrorCode::ResourceBusy => DeviceError::InUse,
            ErrorCode::ResourceOpenReadWrite
                if message.contains("Permission denied") || debug.contains("Permission denied") =>
            {
                DeviceError::Blocked {
                    node: node_in(message).or_else(|| node_in(debug)),
                }
            }
            ErrorCode::ResourceNotFound => DeviceError::Gone,
            ErrorCode::ResourceRead if !still_listed => DeviceError::Gone,
            _ => DeviceError::Failed(message.to_string()),
        }
    }
}

/// The first `/dev/...` path in a text, as v4l2src writes it ("Could not open device
/// '/dev/video0' for reading and writing.").
fn node_in(text: &str) -> Option<String> {
    let start = text.find("/dev/")?;
    let node: String = text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && !matches!(c, '\'' | '"' | ':' | ')' | ','))
        .collect();
    let node = node.trim_end_matches('.');
    (node.len() > "/dev/".len()).then(|| node.to_string())
}

/// What a session reports, on a GStreamer thread.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// The device accepted the format; frames follow.
    Playing {
        format: PixelFormat,
        matrix: Matrix,
    },
    /// The first frame was handed over.
    FirstFrame,
    /// The range in use changed.
    Range(Range),
    Error(DeviceError),
}

type FrameSink = dyn Fn(Frame) + Send + Sync;
type EventSink = dyn Fn(SessionEvent) + Send + Sync;

/// What the streaming thread and the session share.
struct Shared {
    /// The pipeline once built; `None` before and after.
    pipeline: Mutex<Option<gst::Pipeline>>,
    closed: AtomicBool,
    errored: AtomicBool,
    first_frame: AtomicBool,
    detector: Mutex<RangeDetector>,
    /// When the luma was last sampled for the range detector.
    sampled: Mutex<Option<Instant>>,
    session: u64,
    /// The open device's id, to check whether it is still listed when it fails; None for the
    /// test video.
    device_id: Option<String>,
    context: ErrorContext,
    /// When Playing was asked for.
    play_requested: Mutex<Option<Instant>>,
    on_frame: Box<FrameSink>,
    on_event: Box<EventSink>,
}

impl Shared {
    fn event(&self, event: SessionEvent) {
        if !self.closed.load(Ordering::SeqCst) {
            (self.on_event)(event);
        }
    }

    /// Reports one error per session: the first one is what went wrong.
    fn fail(&self, error: DeviceError) {
        if !self.errored.swap(true, Ordering::SeqCst) {
            self.event(SessionEvent::Error(error));
        }
    }
}

/// One open device (or the test video). Closes on [`Session::close`] or drop.
pub struct Session {
    shared: Arc<Shared>,
}

/// The formats the video thread uploads, as GStreamer names.
const UPLOADED: [&str; 6] = ["NV12", "YUY2", "UYVY", "I420", "BGRx", "RGBA"];

/// JPEG decoders, best first (research R4).
const JPEG_DECODERS: [&str; 3] = ["nvjpegdec", "d3d12mjpegdec", "jpegdec"];

/// How the pipeline's source is made.
enum Source {
    Device {
        device: gst::Device,
        mode: Mode,
    },
    /// `frames`: stop after this many (Some(0) never produces one, for the no-signal test).
    Test {
        width: u32,
        height: u32,
        fps: u32,
        frames: Option<u32>,
    },
}

impl Session {
    /// Opens a device in a mode. Returns at once: the pipeline is built and set to Playing on a
    /// short-lived worker (that can block on Windows), and the outcome arrives as events.
    /// `context` maps the device's errors (research R6).
    pub fn open(
        device: &Device,
        mode: &Mode,
        range: RangeChoice,
        session: u64,
        context: ErrorContext,
        on_frame: impl Fn(Frame) + Send + Sync + 'static,
        on_event: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> Self {
        let this = Self::new(
            range,
            session,
            Some(device.id.clone()),
            context,
            Box::new(on_frame),
            Box::new(on_event),
        );
        match &device.gst {
            Some(gst_device) => this.start(Source::Device {
                device: gst_device.clone(),
                mode: mode.clone(),
            }),
            None => this
                .shared
                .fail(DeviceError::Failed(format!("{} has no GStreamer device", device.name))),
        }
        log::info!(
            "session {session}: opening {} ({}) in {} {}",
            device.name,
            device.id,
            mode.text(),
            mode.format.name()
        );
        this
    }

    /// The test video: `videotestsrc is-live=true pattern=ball` in NV12 through the same appsink
    /// path (test_source.rs).
    pub(crate) fn open_test(
        width: u32,
        height: u32,
        fps: u32,
        frames: Option<u32>,
        session: u64,
        on_frame: Box<FrameSink>,
        on_event: Box<EventSink>,
    ) -> Self {
        let this = Self::new(
            RangeChoice::Auto,
            session,
            None,
            ErrorContext::none(),
            on_frame,
            on_event,
        );
        this.start(Source::Test {
            width,
            height,
            fps,
            frames,
        });
        this
    }

    fn new(
        range: RangeChoice,
        session: u64,
        device_id: Option<String>,
        context: ErrorContext,
        on_frame: Box<FrameSink>,
        on_event: Box<EventSink>,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                pipeline: Mutex::new(None),
                closed: AtomicBool::new(false),
                errored: AtomicBool::new(false),
                first_frame: AtomicBool::new(false),
                detector: Mutex::new(RangeDetector::new(range, None)),
                sampled: Mutex::new(None),
                session,
                device_id,
                context,
                play_requested: Mutex::new(None),
                on_frame,
                on_event,
            }),
        }
    }

    fn start(&self, source: Source) {
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new()
            .name(format!("session-{}-open", shared.session))
            .spawn(move || {
                let started = Instant::now();
                let pipeline = match build(&source, &shared) {
                    Ok(pipeline) => pipeline,
                    Err(text) => {
                        log::error!("session {}: {text}", shared.session);
                        shared.fail(DeviceError::Failed(text));
                        return;
                    }
                };
                {
                    let Ok(mut slot) = shared.pipeline.lock() else {
                        return;
                    };
                    if shared.closed.load(Ordering::SeqCst) {
                        return;
                    }
                    *slot = Some(pipeline.clone());
                }
                if let Ok(mut requested) = shared.play_requested.lock() {
                    *requested = Some(Instant::now());
                }
                if let Err(err) = pipeline.set_state(gst::State::Playing) {
                    // The bus carries the reason; this is only the fallback text.
                    log::error!("session {}: cannot play: {err}", shared.session);
                    shared.fail(DeviceError::Failed(err.to_string()));
                }
                if shared.closed.load(Ordering::SeqCst) {
                    let _ = pipeline.set_state(gst::State::Null);
                }
                log::info!(
                    "session {}: set to Playing in {} ms",
                    shared.session,
                    started.elapsed().as_millis()
                );
                drop(pipeline);
                wait_for_signal(&shared);
            });
        if let Err(err) = spawned {
            self.shared
                .fail(DeviceError::Failed(format!("cannot start a thread: {err}")));
        }
    }

    /// Changes the color range choice; the next frame is converted with it, without a restart
    /// (FR-010). A change of the range in use is reported as [`SessionEvent::Range`].
    pub fn set_range(&self, choice: RangeChoice) {
        let changed = {
            let Ok(mut detector) = self.shared.detector.lock() else {
                return;
            };
            let before = detector.in_use();
            detector.set_choice(choice);
            (detector.in_use() != before).then(|| detector.in_use())
        };
        log::info!("session {}: color range {choice:?}", self.shared.session);
        if let Some(range) = changed {
            self.shared.event(SessionEvent::Range(range));
        }
    }

    /// The session number carried by its frames.
    pub fn id(&self) -> u64 {
        self.shared.session
    }

    /// Sets the pipeline to Null; idempotent. No event is sent after it.
    pub fn close(&self) {
        if self.shared.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let pipeline = self.shared.pipeline.lock().ok().and_then(|mut p| p.take());
        if let Some(pipeline) = pipeline {
            let started = Instant::now();
            if let Err(err) = pipeline.set_state(gst::State::Null) {
                log::warn!("session {}: closing failed: {err}", self.shared.session);
            }
            log::info!(
                "session {}: closed in {} ms",
                self.shared.session,
                started.elapsed().as_millis()
            );
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// Builds the pipeline for a source; the text says what is missing.
fn build(source: &Source, shared: &Arc<Shared>) -> Result<gst::Pipeline, String> {
    let pipeline = gst::Pipeline::with_name(&format!("session-{}", shared.session));
    let (src, caps, format) = match source {
        Source::Device { device, mode } => {
            let src = device
                .create_element(Some("source"))
                .map_err(|e| format!("cannot create the device's source: {e}"))?;
            (src, mode.caps(), mode.format.clone())
        }
        Source::Test {
            width,
            height,
            fps,
            frames,
        } => {
            let src = gst::ElementFactory::make("videotestsrc")
                .name("source")
                .property("is-live", true)
                .property_from_str("pattern", "ball")
                .property("num-buffers", frames.map_or(-1, to_i32))
                .build()
                .map_err(|_| "the GStreamer element videotestsrc is missing".to_string())?;
            let caps = gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Nv12)
                .width(to_i32(*width))
                .height(to_i32(*height))
                .framerate(gst::Fraction::new(to_i32(*fps), 1))
                .build();
            (src, caps, Format::Nv12)
        }
    };
    let filter = make("capsfilter")?;
    filter.set_property("caps", &caps);
    pipeline
        .add_many([&src, &filter])
        .and_then(|()| src.link(&filter))
        .map_err(|e| format!("pipeline: {e}"))?;

    let mut chain: Vec<gst::Element> = Vec::new();
    let sink_formats: Vec<&str> = match &format {
        Format::Mjpg => {
            let decoder = JPEG_DECODERS
                .iter()
                .find_map(|name| gst::ElementFactory::make(name).build().ok())
                .ok_or_else(|| "no JPEG decoder available".to_string())?;
            log::info!(
                "session {}: JPEG decoder {}",
                shared.session,
                decoder.factory().map(|f| f.name().to_string()).unwrap_or_default()
            );
            chain.push(decoder);
            // Passthrough when the decoder already gives an uploaded format.
            chain.push(make("videoconvert")?);
            UPLOADED.to_vec()
        }
        Format::Other(name) => {
            log::info!("session {}: {name} converted to NV12", shared.session);
            chain.push(make("videoconvert")?);
            vec!["NV12"]
        }
        raw => vec![raw.name()],
    };
    let sink = appsink(&sink_formats, shared);
    let mut previous = filter;
    for element in chain.iter().chain(std::iter::once(sink.upcast_ref())) {
        pipeline
            .add(element)
            .and_then(|()| previous.link(element))
            .map_err(|e| format!("pipeline: {e}"))?;
        previous = element.clone();
    }

    let weak = Arc::downgrade(shared);
    if let Some(bus) = pipeline.bus() {
        bus.set_sync_handler(move |_, message| {
            if let Some(shared) = weak.upgrade() {
                on_message(&shared, message);
            }
            gst::BusSyncReply::Drop
        });
    }
    Ok(pipeline)
}

fn make(name: &str) -> Result<gst::Element, String> {
    gst::ElementFactory::make(name)
        .build()
        .map_err(|_| format!("the GStreamer element {name} is missing"))
}

fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// Waits on the open worker until the first frame, an error or the close; no frame within
/// [`NO_SIGNAL_AFTER`] → [`DeviceError::NoSignal`].
fn wait_for_signal(shared: &Shared) {
    const STEP: Duration = Duration::from_millis(50);
    let deadline = Instant::now() + NO_SIGNAL_AFTER;
    let settled = || {
        shared.closed.load(Ordering::SeqCst)
            || shared.errored.load(Ordering::SeqCst)
            || shared.first_frame.load(Ordering::SeqCst)
    };
    while !settled() && Instant::now() < deadline {
        std::thread::sleep(STEP);
    }
    if !settled() {
        log::warn!(
            "session {}: no frame within {} s",
            shared.session,
            NO_SIGNAL_AFTER.as_secs()
        );
        shared.fail(DeviceError::NoSignal);
    }
}

/// Logs every error and warning with its debug text; the first error ends the session, mapped
/// to why the device cannot play (research R6).
fn on_message(shared: &Shared, message: &gst::Message) {
    match message.view() {
        gst::MessageView::Error(err) => {
            let debug = err.debug().map(|d| d.to_string()).unwrap_or_default();
            log::error!(
                "session {}: {} ({debug}) from {:?}",
                shared.session,
                err.error(),
                err.src().map(|s| s.path_string())
            );
            if shared.errored.load(Ordering::SeqCst) || shared.closed.load(Ordering::SeqCst) {
                return;
            }
            let since_playing = shared
                .play_requested
                .lock()
                .ok()
                .and_then(|requested| *requested)
                .map(|at| at.elapsed());
            let access = (shared.context.access)();
            // Read again from the providers: the monitor's own list may lag the device leaving.
            let still_listed = shared
                .device_id
                .as_ref()
                .is_none_or(|id| crate::monitor::listed_now().iter().any(|device| &device.id == id));
            let error = map_error(
                shared.context.windows,
                ErrorCode::of(&err.error()),
                &err.error().to_string(),
                &debug,
                since_playing,
                still_listed,
                access,
            );
            log::info!(
                "session {}: mapped to {error:?} (since playing {since_playing:?}, listed {still_listed}, access {access:?})",
                shared.session
            );
            shared.fail(error);
        }
        gst::MessageView::Warning(warning) => {
            // v4l2src warns here when it judges the driver's timestamps bad (research R11).
            log::warn!(
                "session {}: {} ({})",
                shared.session,
                warning.error(),
                warning.debug().map(|d| d.to_string()).unwrap_or_default()
            );
        }
        _ => {}
    }
}

/// An appsink that never waits (`sync=false max-buffers=1 drop=true`), accepting the given raw
/// formats in system memory, that copies each frame's planes into pooled buffers.
fn appsink(formats: &[&str], shared: &Arc<Shared>) -> gst_app::AppSink {
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", gst::List::new(formats.iter().copied()))
        .build();
    let sink = gst_app::AppSink::builder()
        .caps(&caps)
        .sync(false)
        .max_buffers(1)
        .drop(true)
        .build();

    // Playing: the device accepted a format and frames follow.
    if let Some(pad) = sink.static_pad("sink") {
        let weak = Arc::downgrade(shared);
        pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
            if let Some(gst::PadProbeData::Event(event)) = &info.data
                && let gst::EventView::Caps(caps) = event.view()
                && let Some(shared) = weak.upgrade()
                && let Ok(video) = gst_video::VideoInfo::from_caps(caps.caps())
                && let Some(format) = pixel_format(video.format())
            {
                let matrix = frame::matrix_for(matrix_of(&video), video.height());
                let range = shared.detector.lock().ok().map(|mut detector| {
                    detector.reset(range_of(&video));
                    detector.in_use()
                });
                log::info!(
                    "session {}: playing {}x{} {} {:?} declared range {:?}",
                    shared.session,
                    video.width(),
                    video.height(),
                    format.name(),
                    matrix,
                    range_of(&video)
                );
                shared.event(SessionEvent::Playing { format, matrix });
                // The range in use from the start; later changes follow.
                if let Some(range) = range {
                    shared.event(SessionEvent::Range(range));
                }
            }
            gst::PadProbeReturn::Ok
        });
    }

    let weak = Arc::downgrade(shared);
    let pool = PlanePool::new();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let shared = weak.upgrade().ok_or(gst::FlowError::Flushing)?;
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let frame = copy_frame(sink, &sample, &pool, &shared)?;
                detect_range(&shared, &frame);
                if !shared.first_frame.swap(true, Ordering::SeqCst) {
                    shared.event(SessionEvent::FirstFrame);
                }
                if !shared.closed.load(Ordering::SeqCst) {
                    (shared.on_frame)(frame);
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    sink
}

/// How often the luma is sampled for the range detector (research R5).
const SAMPLE_EVERY: Duration = Duration::from_millis(500);

/// Samples a frame's luma at most every [`SAMPLE_EVERY`], on the streaming thread (never on the
/// video thread, FR-015), and reports a change of the range in use; the next frame carries it.
fn detect_range(shared: &Shared, frame: &Frame) {
    let Ok(mut detector) = shared.detector.lock() else {
        return;
    };
    if !detector.wants_samples() {
        return;
    }
    let now = Instant::now();
    if let Ok(mut sampled) = shared.sampled.lock() {
        if sampled.is_some_and(|at| now < at + SAMPLE_EVERY) {
            return;
        }
        *sampled = Some(now);
    }
    let Some(plane) = frame.planes().first() else {
        return;
    };
    let samples = analysis::sample_luma(
        plane.bytes.as_slice(),
        plane.stride,
        frame.width,
        frame.height,
        frame.format,
    );
    if detector.observe(&samples) {
        let range = detector.in_use();
        drop(detector);
        log::info!("session {}: range in use {range:?} (inferred)", shared.session);
        shared.event(SessionEvent::Range(range));
    }
}

/// Copies a sample's planes, rows packed, into a pooled frame stamped with its capture time.
fn copy_frame(
    sink: &gst_app::AppSink,
    sample: &gst::Sample,
    pool: &PlanePool,
    shared: &Shared,
) -> Result<Frame, gst::FlowError> {
    let now = Instant::now();
    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
    let info = sample
        .caps()
        .and_then(|c| gst_video::VideoInfo::from_caps(c).ok())
        .ok_or(gst::FlowError::NotNegotiated)?;
    let format = pixel_format(info.format()).ok_or(gst::FlowError::NotNegotiated)?;
    let video = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).map_err(|_| gst::FlowError::Error)?;
    let (width, height) = (video.width(), video.height());

    let mut planes = Vec::with_capacity(3);
    for (index, (row, rows)) in format.plane_sizes(width, height).into_iter().enumerate() {
        let src = video.plane_data(index as u32).map_err(|_| gst::FlowError::Error)?;
        let stride = usize::try_from(video.plane_stride()[index]).map_err(|_| gst::FlowError::Error)?;
        let mut bytes = pool.take(row * rows);
        for (y, dst) in bytes.as_mut_slice().chunks_exact_mut(row).enumerate() {
            let start = y * stride;
            let line = src.get(start..start + row).ok_or(gst::FlowError::Error)?;
            dst.copy_from_slice(line);
        }
        planes.push(Plane { bytes, stride: row });
    }

    let captured = match (sink.clock(), buffer.pts(), sink.base_time()) {
        (Some(clock), Some(pts), Some(base)) => {
            frame::captured_at(now, clock.time().nseconds(), pts.nseconds(), base.nseconds())
        }
        _ => now,
    };
    let range = shared.detector.lock().map(|d| d.in_use()).unwrap_or(Range::Limited);
    Ok(Frame::new(
        width,
        height,
        format,
        planes,
        frame::matrix_for(matrix_of(&info), height),
        range,
        captured,
        Instant::now(),
        shared.session,
    ))
}

fn pixel_format(format: gst_video::VideoFormat) -> Option<PixelFormat> {
    use gst_video::VideoFormat as V;
    Some(match format {
        V::Nv12 => PixelFormat::Nv12,
        V::Yuy2 => PixelFormat::Yuy2,
        V::Uyvy => PixelFormat::Uyvy,
        V::I420 => PixelFormat::I420,
        V::Bgrx => PixelFormat::Bgrx,
        V::Rgba => PixelFormat::Rgba,
        _ => return None,
    })
}

fn matrix_of(info: &gst_video::VideoInfo) -> Option<Matrix> {
    match info.colorimetry().matrix() {
        gst_video::VideoColorMatrix::Bt709 => Some(Matrix::Bt709),
        gst_video::VideoColorMatrix::Bt601 => Some(Matrix::Bt601),
        _ => None,
    }
}

fn range_of(info: &gst_video::VideoInfo) -> Option<Range> {
    match info.colorimetry().range() {
        gst_video::VideoColorRange::Range0_255 => Some(Range::Full),
        gst_video::VideoColorRange::Range16_235 => Some(Range::Limited),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOON: Option<Duration> = Some(Duration::from_millis(300));
    const LATE: Option<Duration> = Some(Duration::from_secs(30));

    fn windows(code: ErrorCode, since: Option<Duration>, listed: bool, access: Access) -> DeviceError {
        map_error(true, code, "Internal data stream error.", "", since, listed, access)
    }

    fn linux(code: ErrorCode, message: &str, debug: &str, listed: bool) -> DeviceError {
        map_error(false, code, message, debug, SOON, listed, Access::Unknown)
    }

    #[test]
    fn windows_denied_access_is_blocked_before_anything_else() {
        let error = windows(ErrorCode::StreamFailed, SOON, false, Access::Denied);
        assert_eq!(error, DeviceError::Blocked { node: None });
    }

    #[test]
    fn windows_device_not_listed_is_gone() {
        for access in [Access::Allowed, Access::Unknown] {
            assert_eq!(windows(ErrorCode::StreamFailed, SOON, false, access), DeviceError::Gone);
        }
    }

    #[test]
    fn windows_stream_failed_soon_after_playing_is_in_use() {
        let error = windows(ErrorCode::StreamFailed, SOON, true, Access::Allowed);
        assert_eq!(error, DeviceError::InUse);
        let edge = windows(ErrorCode::StreamFailed, Some(IN_USE_WITHIN), true, Access::Allowed);
        assert_eq!(edge, DeviceError::InUse);
    }

    #[test]
    fn windows_other_errors_fail_with_the_text() {
        let failed = DeviceError::Failed("Internal data stream error.".into());
        // Late, before Playing, or another code.
        assert_eq!(windows(ErrorCode::StreamFailed, LATE, true, Access::Allowed), failed);
        assert_eq!(windows(ErrorCode::StreamFailed, None, true, Access::Allowed), failed);
        assert_eq!(windows(ErrorCode::Other, SOON, true, Access::Allowed), failed);
        // Linux codes mean nothing special on Windows.
        assert_eq!(windows(ErrorCode::ResourceBusy, SOON, true, Access::Unknown), failed);
    }

    #[test]
    fn linux_busy_is_in_use() {
        assert_eq!(
            linux(ErrorCode::ResourceBusy, "Device is busy", "", true),
            DeviceError::InUse
        );
    }

    #[test]
    fn linux_permission_denied_is_blocked_naming_the_node() {
        let error = linux(
            ErrorCode::ResourceOpenReadWrite,
            "Could not open device '/dev/video2' for reading and writing.",
            "../sys/v4l2/v4l2_calls.c(637): gst_v4l2_open (): system error: Permission denied",
            true,
        );
        assert_eq!(
            error,
            DeviceError::Blocked {
                node: Some("/dev/video2".into())
            }
        );
        // The node from the debug text when the message lacks it.
        let error = linux(
            ErrorCode::ResourceOpenReadWrite,
            "Could not open device.",
            "Cannot open /dev/video0: Permission denied",
            true,
        );
        assert_eq!(
            error,
            DeviceError::Blocked {
                node: Some("/dev/video0".into())
            }
        );
    }

    #[test]
    fn linux_open_failure_without_permission_text_fails() {
        let error = linux(
            ErrorCode::ResourceOpenReadWrite,
            "Could not open device.",
            "No such device",
            true,
        );
        assert_eq!(error, DeviceError::Failed("Could not open device.".into()));
    }

    #[test]
    fn linux_not_found_is_gone() {
        assert_eq!(
            linux(ErrorCode::ResourceNotFound, "Cannot identify device", "", true),
            DeviceError::Gone
        );
    }

    #[test]
    fn linux_read_error_is_gone_only_when_no_longer_listed() {
        assert_eq!(
            linux(ErrorCode::ResourceRead, "Failed to read", "", false),
            DeviceError::Gone
        );
        assert_eq!(
            linux(ErrorCode::ResourceRead, "Failed to read", "", true),
            DeviceError::Failed("Failed to read".into())
        );
    }

    #[test]
    fn linux_other_errors_fail_and_ignore_access() {
        let error = map_error(false, ErrorCode::StreamFailed, "boom", "", SOON, true, Access::Denied);
        assert_eq!(error, DeviceError::Failed("boom".into()));
    }

    #[test]
    fn node_in_texts() {
        assert_eq!(node_in("device '/dev/video10' busy").as_deref(), Some("/dev/video10"));
        assert_eq!(node_in("at /dev/video3.").as_deref(), Some("/dev/video3"));
        assert_eq!(node_in("(/dev/video1)").as_deref(), Some("/dev/video1"));
        assert_eq!(node_in("no node here"), None);
        assert_eq!(node_in("just /dev/ alone"), None);
    }

    #[test]
    fn error_codes_from_gstreamer_errors() {
        let of = |e: gst::glib::Error| ErrorCode::of(&e);
        assert_eq!(
            of(gst::glib::Error::new(gst::ResourceError::Busy, "x")),
            ErrorCode::ResourceBusy
        );
        assert_eq!(
            of(gst::glib::Error::new(gst::ResourceError::OpenReadWrite, "x")),
            ErrorCode::ResourceOpenReadWrite
        );
        assert_eq!(
            of(gst::glib::Error::new(gst::ResourceError::NotFound, "x")),
            ErrorCode::ResourceNotFound
        );
        assert_eq!(
            of(gst::glib::Error::new(gst::ResourceError::Read, "x")),
            ErrorCode::ResourceRead
        );
        assert_eq!(
            of(gst::glib::Error::new(gst::StreamError::Failed, "x")),
            ErrorCode::StreamFailed
        );
        assert_eq!(
            of(gst::glib::Error::new(gst::CoreError::StateChange, "x")),
            ErrorCode::Other
        );
        assert_eq!(
            of(gst::glib::Error::new(gst::ResourceError::Write, "x")),
            ErrorCode::Other
        );
    }
}
