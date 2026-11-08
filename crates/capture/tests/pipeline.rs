//! The session's pipeline with `videotestsrc` (no device needed): NV12 frames reach `on_frame`
//! with their planes and times, and events arrive in order. Skipped with a message when
//! GStreamer cannot start.

use std::sync::mpsc;
use std::time::Duration;

use capturax_capture::{DeviceError, Frame, PixelFormat, Range, RangeChoice, Session, SessionEvent};

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
                SessionEvent::Range(_),
                SessionEvent::FirstFrame
            ]
        ),
        "events: {order:?}"
    );
    session.close();
    session.close();
}

#[test]
fn set_range_applies_to_the_next_frames_without_a_restart() {
    if !gstreamer_available() {
        return;
    }
    let (session, rx) = test_video(3);
    let next_frame = || loop {
        match rx.recv_timeout(Duration::from_secs(10)).expect("the test video plays") {
            Seen::Frame(frame) => return frame,
            Seen::Event(_) => {}
        }
    };
    // The test video declares no range: Auto uses Limited (the ball's black is not below 16).
    assert_eq!(next_frame().range, Range::Limited);
    session.set_range(RangeChoice::Full);
    // A frame already copied may still carry the old range; the ones after carry the new one.
    let mut events = Vec::new();
    let mut full = false;
    for _ in 0..5 {
        let frame = loop {
            match rx.recv_timeout(Duration::from_secs(10)).expect("the test video plays") {
                Seen::Frame(frame) => break frame,
                Seen::Event(event) => events.push(event),
            }
        };
        full = frame.range == Range::Full;
    }
    assert!(full, "frames after set_range(Full) are Full");
    assert!(events.contains(&SessionEvent::Range(Range::Full)), "events: {events:?}");
    assert!(
        !events.iter().any(|e| matches!(e, SessionEvent::Playing { .. })),
        "no restart: {events:?}"
    );
    session.close();
}

#[test]
fn no_frame_within_five_seconds_is_no_signal() {
    if !gstreamer_available() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let started = std::time::Instant::now();
    let session = Session::test_without_frames(3, move |event| {
        let _ = tx.lock().unwrap().send(event);
    });
    let error = loop {
        match rx.recv_timeout(Duration::from_secs(10)).expect("an error within 10 s") {
            SessionEvent::Error(error) => break error,
            SessionEvent::FirstFrame => panic!("num-buffers=0 gave a frame"),
            _ => {}
        }
    };
    let waited = started.elapsed();
    assert_eq!(error, DeviceError::NoSignal);
    assert!(waited >= Duration::from_secs(5), "after {waited:?}");
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "one error only");
    session.close();
}
