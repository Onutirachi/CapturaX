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
    Device { device: gst::Device, mode: Mode },
    Test { width: u32, height: u32, fps: u32 },
}

impl Session {
    /// Opens a device in a mode. Returns at once: the pipeline is built and set to Playing on a
    /// short-lived worker (that can block on Windows), and the outcome arrives as events.
    pub fn open(
        device: &Device,
        mode: &Mode,
        range: RangeChoice,
        session: u64,
        on_frame: impl Fn(Frame) + Send + Sync + 'static,
        on_event: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> Self {
        let this = Self::new(range, session, Box::new(on_frame), Box::new(on_event));
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
            mode.label(),
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
        session: u64,
        on_frame: Box<FrameSink>,
        on_event: Box<EventSink>,
    ) -> Self {
        let this = Self::new(RangeChoice::Auto, session, on_frame, on_event);
        this.start(Source::Test { width, height, fps });
        this
    }

    fn new(range: RangeChoice, session: u64, on_frame: Box<FrameSink>, on_event: Box<EventSink>) -> Self {
        Self {
            shared: Arc::new(Shared {
                pipeline: Mutex::new(None),
                closed: AtomicBool::new(false),
                errored: AtomicBool::new(false),
                first_frame: AtomicBool::new(false),
                detector: Mutex::new(RangeDetector::new(range, None)),
                sampled: Mutex::new(None),
                session,
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
        Source::Test { width, height, fps } => {
            let src = gst::ElementFactory::make("videotestsrc")
                .name("source")
                .property("is-live", true)
                .property_from_str("pattern", "ball")
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

/// Logs every error and warning with its debug text; the first error ends the session.
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
            shared.fail(DeviceError::Failed(err.error().to_string()));
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
