//! The session's pipeline with `videotestsrc` (no device needed): NV12 frames reach `on_frame`
//! with their planes and times, and events arrive in order. Skipped with a message when
//! GStreamer cannot start.

use std::sync::mpsc;
use std::time::Duration;

use capturax_capture::{Frame, PixelFormat, Session, SessionEvent};

/// What the callbacks saw, in order.
enum Seen {
    Event(SessionEvent),
    Frame(Frame),
}

fn gstreamer_available() -> bool {
    if let Err(err) = gst::init() {
        eprintln!("skipped: GStreamer cannot start ({err})");
        return false;
    }
    if gst::ElementFactory::find("videotestsrc").is_none() {
        eprintln!("skipped: videotestsrc is missing");
        return false;
    }
    true
}

fn test_video(session: u64) -> (Session, mpsc::Receiver<Seen>) {
    let (tx, rx) = mpsc::channel();
    let frames = tx.clone();
    let frames = std::sync::Mutex::new(frames);
    let events = std::sync::Mutex::new(tx);
    let session = Session::test_video(
        640,
        360,
        30,
        session,
        move |frame| {
            let _ = frames.lock().unwrap().send(Seen::Frame(frame));
        },
        move |event| {
            let _ = events.lock().unwrap().send(Seen::Event(event));
        },
    );
    (session, rx)
}

#[test]
fn nv12_frames_and_events_in_order() {
    if !gstreamer_available() {
        return;
    }
    let (session, rx) = test_video(7);
    let mut order = Vec::new();
    let mut frames = 0;
    while frames < 5 {
        match rx.recv_timeout(Duration::from_secs(10)).expect("the test video plays") {
            Seen::Event(event) => order.push(event),
            Seen::Frame(frame) => {
                assert_eq!(frame.format, PixelFormat::Nv12);
                assert_eq!((frame.width, frame.height), (640, 360));
                assert_eq!(frame.session, 7);
                let planes = frame.planes();
                assert_eq!(planes.len(), 2);
                assert_eq!(planes[0].bytes.len(), 640 * 360);
                assert_eq!(planes[0].stride, 640);
                assert_eq!(planes[1].bytes.len(), 640 * 180);
                assert!(frame.captured <= frame.ready, "captured before ready");
                frames += 1;
            }
        }
    }
    assert!(
        matches!(
            order.as_slice(),
            [
                SessionEvent::Playing {
                    format: PixelFormat::Nv12,
                    ..
                },
                SessionEvent::FirstFrame
            ]
        ),
        "events: {order:?}"
    );
    session.close();
    session.close();
}
