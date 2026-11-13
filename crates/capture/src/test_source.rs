//! The synthetic test video (app shell FR-004) as a capture source like any other: a moving
//! `videotestsrc` in NV12 through the session's appsink, so the YUV path is the one measured
//! (research R12).

use crate::Unavailable;
use crate::frame::Frame;
use crate::session::{Session, SessionEvent};

/// Starts GStreamer and checks that `videotestsrc` exists, so a missing plugin is known before
/// the test video opens (exit code 69, contracts/cli.md).
pub fn available() -> Result<(), Unavailable> {
    gst::init().map_err(|e| Unavailable(format!("GStreamer: {e}")))?;
    if gst::ElementFactory::find("videotestsrc").is_none() {
        return Err(Unavailable("the GStreamer element videotestsrc is missing".into()));
    }
    Ok(())
}

impl Session {
    /// `videotestsrc is-live=true pattern=ball` at this size and rate, in NV12. Returns at once,
    /// like [`Session::open`].
    pub fn test_video(
        width: u32,
        height: u32,
        fps: u32,
        session: u64,
        on_frame: impl Fn(Frame) + Send + Sync + 'static,
        on_event: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> Self {
        log::info!("session {session}: test video {width}x{height}@{fps}");
        Self::open_test(
            width,
            height,
            fps,
            None,
            session,
            Box::new(on_frame),
            Box::new(on_event),
        )
    }

    /// A test video that never produces a frame (`num-buffers=0`), for the no-signal check.
    #[doc(hidden)]
    pub fn test_without_frames(session: u64, on_event: impl Fn(SessionEvent) + Send + Sync + 'static) -> Self {
        Self::open_test(640, 360, 30, Some(0), session, Box::new(|_| {}), Box::new(on_event))
    }
}
