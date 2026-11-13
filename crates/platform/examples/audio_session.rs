//! Audio session check for the preview audio (spec 20261113-103922-preview-audio, research R2,
//! R9): opens an input and an output through `capturax_platform::audio::Session`, passes the
//! samples straight through (no bridge: the input's channels copied 1:1 while the rates match,
//! silence otherwise), and prints the session's events, the delivered rates each second, a
//! reopen of the output halfway, and how long the close takes to report `Closed`.
//!
//! `cargo run -p capturax-platform --example audio_session -- <input id> [options]`
//!
//! - `--output <id>`: the output endpoint (default: the system default output)
//! - `--exclusive-in`, `--exclusive-out`: try the device alone first
//! - `--seconds <n>`: how long to play (default 10)
//!
//! Ids as `audio_endpoints` or the `audio` monitor print them.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use capturax_platform::audio::{Format, Samples, Session, SessionEvent, SessionRequest};

/// The input's samples waiting for the output, at most 100 ms.
#[derive(Default)]
struct Passthrough {
    queue: Mutex<(VecDeque<f32>, Option<Format>)>,
    input_frames: AtomicU64,
    output_frames: AtomicU64,
    silent_frames: AtomicU64,
}

impl Samples for Passthrough {
    fn on_input(&self, samples: &[f32], format: Format) {
        let channels = usize::from(format.channels);
        self.input_frames
            .fetch_add((samples.len() / channels) as u64, Ordering::Relaxed);
        let mut queue = self.queue.lock().unwrap();
        queue.1 = Some(format);
        queue.0.extend(samples);
        let cap = format.rate as usize / 10 * channels;
        let excess = queue.0.len().saturating_sub(cap);
        queue.0.drain(..excess);
    }

    fn fill(&self, out: &mut [f32], format: Format) {
        let out_channels = usize::from(format.channels);
        let frames = out.len() / out_channels;
        self.output_frames.fetch_add(frames as u64, Ordering::Relaxed);
        let mut queue = self.queue.lock().unwrap();
        let Some(input) = queue.1.filter(|f| f.rate == format.rate) else {
            self.silent_frames.fetch_add(frames as u64, Ordering::Relaxed);
            return;
        };
        let in_channels = usize::from(input.channels);
        let available = (queue.0.len() / in_channels).min(frames);
        for (frame, samples) in out.chunks_exact_mut(out_channels).zip(
            queue
                .0
                .drain(..available * in_channels)
                .collect::<Vec<_>>()
                .chunks_exact(in_channels),
        ) {
            for (c, value) in frame.iter_mut().enumerate() {
                *value = samples[c.min(in_channels - 1)];
            }
        }
        self.silent_frames
            .fetch_add((frames - available) as u64, Ordering::Relaxed);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut request = SessionRequest {
        number: 1,
        input: String::new(),
        output: None,
        input_exclusive: false,
        output_exclusive: false,
    };
    let mut seconds = 10u64;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => request.output = args.next(),
            "--exclusive-in" => request.input_exclusive = true,
            "--exclusive-out" => request.output_exclusive = true,
            "--seconds" => seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(seconds),
            _ => request.input = arg,
        }
    }
    if request.input.is_empty() {
        eprintln!("usage: audio_session <input id> [--output <id>] [--exclusive-in] [--exclusive-out] [--seconds <n>]");
        std::process::exit(2);
    }

    let samples = Arc::new(Passthrough::default());
    let (closed, wait_closed) = mpsc::channel();
    let started = Instant::now();
    let output = request.output.clone();
    let output_exclusive = request.output_exclusive;
    let mut session = Session::open(request, Arc::clone(&samples) as Arc<dyn Samples>, move |event| {
        println!("{:>6.3} s  {event:?}", started.elapsed().as_secs_f64());
        if let SessionEvent::Closed { .. } = event {
            let _ = closed.send(Instant::now());
        }
    });
    println!("open returned after {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);

    let mut last = (0, 0, 0);
    for second in 1..=seconds {
        std::thread::sleep(Duration::from_secs(1));
        let now = (
            samples.input_frames.load(Ordering::Relaxed),
            samples.output_frames.load(Ordering::Relaxed),
            samples.silent_frames.load(Ordering::Relaxed),
        );
        println!(
            "{second:>4} s  input {} frames/s, output {} frames/s, silence {} frames",
            now.0 - last.0,
            now.1 - last.1,
            now.2 - last.2
        );
        last = now;
        if second == seconds / 2 {
            println!("reopening the output");
            session.reopen_output(output.clone(), output_exclusive);
        }
    }

    let closing = Instant::now();
    session.close();
    println!(
        "close returned after {:.3} ms",
        closing.elapsed().as_secs_f64() * 1000.0
    );
    match wait_closed.recv_timeout(Duration::from_secs(2)) {
        Ok(at) => println!("Closed after {:.1} ms", (at - closing).as_secs_f64() * 1000.0),
        Err(_) => println!("no Closed within 2 s"),
    }
}
