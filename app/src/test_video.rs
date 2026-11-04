//! The synthetic test video (spec FR-004, research R9): a moving GStreamer `videotestsrc` handed
//! to the video thread through the same appsink path spec #3 uses for capture devices.

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use gst::prelude::*;
use gst_video::prelude::*;

use crate::cli::VideoSize;
use crate::render::{Inbox, VideoFrame};

/// GStreamer cannot start or its `videotestsrc` plugin is missing (exit code 69, contracts/cli.md).
#[derive(Debug)]
pub struct Unavailable(pub String);

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "test video unavailable: {}", self.0)
    }
}

/// The running test video; `stop` sets the pipeline to Null.
pub struct TestVideo {
    pipeline: gst::Pipeline,
    /// Paces `videotestsrc` evenly on Windows (research R9).
    _timers: platform::TimerGuard,
}

impl TestVideo {
    /// Initializes GStreamer and plays the pipeline
    /// `videotestsrc is-live=true pattern=ball ! video/x-raw,format=RGBA,... ! appsink`.
    pub fn start(size: VideoSize, inbox: Arc<Inbox>) -> Result<Self, Unavailable> {
        let timers = platform::precise_timers();
        gst::init().map_err(|e| Unavailable(format!("GStreamer: {e}")))?;
        let source = gst::ElementFactory::make("videotestsrc")
            .property("is-live", true)
            .property_from_str("pattern", "ball")
            .build()
            .map_err(|_| Unavailable("the GStreamer element videotestsrc is missing".into()))?;
        let caps = gst_video::VideoCapsBuilder::new()
            .format(gst_video::VideoFormat::Rgba)
            .width(to_i32(size.width))
            .height(to_i32(size.height))
            .framerate(gst::Fraction::new(to_i32(size.fps), 1))
            .build();
        let sink = appsink(caps, inbox);
        let pipeline = gst::Pipeline::with_name("test-video");
        pipeline
            .add_many([&source, sink.upcast_ref()])
            .and_then(|()| source.link(&sink))
            .map_err(|e| Unavailable(format!("pipeline: {e}")))?;
        if let Some(bus) = pipeline.bus() {
            bus.set_sync_handler(|_, message| {
                if let gst::MessageView::Error(err) = message.view() {
                    log::error!("test video: {} ({:?})", err.error(), err.debug());
                }
                gst::BusSyncReply::Drop
            });
        }
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| Unavailable(format!("cannot play: {e}")))?;
        log::info!("test video {}x{}@{} playing", size.width, size.height, size.fps);
        Ok(Self {
            pipeline,
            _timers: timers,
        })
    }

    pub fn stop(&mut self) {
        if let Err(err) = self.pipeline.set_state(gst::State::Null) {
            log::warn!("test video: stopping failed: {err}");
        }
    }
}

impl Drop for TestVideo {
    fn drop(&mut self) {
        self.stop();
    }
}

fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// An appsink that never waits (`sync=false max-buffers=1 drop=true`) and puts each RGBA frame,
/// rows packed, in the inbox with the time it became ready: when it is handed over, as the spike
/// measured it, so the source's own copy is not part of SC-001.
fn appsink(caps: gst::Caps, inbox: Arc<Inbox>) -> gst_app::AppSink {
    let sink = gst_app::AppSink::builder()
        .caps(&caps)
        .sync(false)
        .max_buffers(1)
        .drop(true)
        .build();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer_owned().ok_or(gst::FlowError::Error)?;
                let info = sample
                    .caps()
                    .and_then(|c| gst_video::VideoInfo::from_caps(c).ok())
                    .ok_or(gst::FlowError::NotNegotiated)?;
                let frame =
                    gst_video::VideoFrame::from_buffer_readable(buffer, &info).map_err(|_| gst::FlowError::Error)?;
                let (width, height) = (frame.width(), frame.height());
                let stride = usize::try_from(frame.plane_stride()[0]).map_err(|_| gst::FlowError::Error)?;
                let row = width as usize * 4;
                let src = frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
                let mut data = vec![0u8; row * height as usize];
                for (y, dst) in data.chunks_exact_mut(row).enumerate() {
                    dst.copy_from_slice(&src[y * stride..y * stride + row]);
                }
                inbox.send_frame(VideoFrame {
                    width,
                    height,
                    data,
                    ready: Instant::now(),
                });
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    sink
}
