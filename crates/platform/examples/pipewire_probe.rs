//! Linux engine probe for the preview audio (spec 20261113-103922-preview-audio, research R3,
//! quickstart "Linux engine probe"): plays a capture card's PipeWire node on the default output
//! through two streams in one node group, the way the app will, and prints once a second:
//!
//! - `delivered_ppm`: input frames received against wall time since the first input buffer
//!   (the spike's starvation showed as −750000 to −830000);
//! - `played_ppm`: the same for the output; `delivered_ppm − played_ppm` is the drift the bridge's
//!   follower would correct (quickstart step 4: within ±50 ppm);
//! - `gap_ms` / `drop_ms`: silence played for lack of input / input dropped above 40 ms waiting,
//!   in the last second;
//! - `wait_ms` / `wait_max_ms`: the lowest and highest input waiting at an output cycle in the
//!   last second; `in_period` / `out_period`: the last buffer sizes in frames.
//!
//! On `SIGINT` or `SIGTERM` it disconnects both streams, quits the loop and exits 0; a watchdog
//! exits 2 if that takes longer than 1 s (the spike's `pipewiresink` hung there).
//!
//! `cargo run --release -p capturax-platform --example pipewire_probe [-- --input <text>] [--latency <frames>]`
//!
//! `--input` picks the first `Audio/Source` whose description contains the text (default
//! `ezcap`); `--latency` sets `node.latency` to `<frames>/48000` (default 128). The quickstart's
//! runs use the built binary, `target/release/examples/pipewire_probe`.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("pipewire_probe runs on Linux only");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use pipewire as pw;
    use pw::properties::properties;
    use pw::spa;
    use pw::spa::param::audio::{AudioFormat, AudioInfoRaw, MAX_CHANNELS};
    use pw::spa::param::format::{MediaSubtype, MediaType};
    use pw::spa::param::format_utils;
    use pw::spa::pod::Pod;
    use pw::types::ObjectType;

    const RATE: u32 = 48_000;
    const CHANNELS: usize = 2;
    const STRIDE: usize = CHANNELS * size_of::<f32>();
    const GROUP: &str = "capturax.preview";
    /// The bridge's margin (research R4): input kept waiting after priming.
    const MARGIN_FRAMES: u64 = RATE as u64 * 2 / 1000;
    /// Above this much waiting input the oldest is dropped (research R4).
    const DROP_ABOVE_FRAMES: u64 = RATE as u64 * 40 / 1000;

    static START: OnceLock<Instant> = OnceLock::new();

    macro_rules! say {
        ($($arg:tt)*) => {
            eprintln!("{:8.3} {}", START.get_or_init(Instant::now).elapsed().as_secs_f64(), format!($($arg)*))
        };
    }

    struct Args {
        input: String,
        latency: u32,
    }

    fn args() -> Args {
        let usage = "usage: pipewire_probe [--input <text>] [--latency <frames>]";
        let (mut input, mut latency) = ("ezcap".to_owned(), 128);
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--input" => input = args.next().unwrap_or_else(|| fail(usage)),
                "--latency" => {
                    latency = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| fail(usage));
                }
                _ => fail(usage),
            }
        }
        Args { input, latency }
    }

    fn fail(message: &str) -> ! {
        say!("{message}");
        std::process::exit(1);
    }

    /// What the two `process` callbacks (PipeWire's data thread) and the stats timer share.
    struct Shared {
        ring: Mutex<VecDeque<f32>>,
        primed: AtomicBool,
        first_input: OnceLock<Instant>,
        first_output: OnceLock<Instant>,
        input_frames: AtomicU64,
        output_frames: AtomicU64,
        in_period: AtomicU64,
        out_period: AtomicU64,
        // Reset every second by the stats.
        gap_frames: AtomicU64,
        drop_frames: AtomicU64,
        wait_min: AtomicU64,
        wait_max: AtomicU64,
    }

    impl Shared {
        fn new() -> Self {
            Self {
                ring: Mutex::new(VecDeque::with_capacity(RATE as usize * CHANNELS)),
                primed: AtomicBool::new(false),
                first_input: OnceLock::new(),
                first_output: OnceLock::new(),
                input_frames: AtomicU64::new(0),
                output_frames: AtomicU64::new(0),
                in_period: AtomicU64::new(0),
                out_period: AtomicU64::new(0),
                gap_frames: AtomicU64::new(0),
                drop_frames: AtomicU64::new(0),
                wait_min: AtomicU64::new(u64::MAX),
                wait_max: AtomicU64::new(0),
            }
        }

        /// Capture side: appends one buffer of interleaved F32 stereo.
        fn push(&self, bytes: &[u8]) {
            let frames = (bytes.len() / STRIDE) as u64;
            if frames == 0 {
                return;
            }
            self.first_input.get_or_init(Instant::now);
            self.input_frames.fetch_add(frames, Ordering::Relaxed);
            self.in_period.store(frames, Ordering::Relaxed);
            let mut ring = self.ring.lock().expect("ring lock");
            let samples = bytes[..frames as usize * STRIDE].as_chunks::<4>().0;
            ring.extend(samples.iter().map(|b| f32::from_le_bytes(*b)));
        }

        /// Playback side: fills `out` (interleaved F32 stereo bytes) from the ring, as the bridge
        /// will without its rate follower: primes to one input period plus the margin, drops above
        /// 40 ms, plays silence (a gap once primed) when short.
        fn pull(&self, out: &mut [u8]) {
            let frames = (out.len() / STRIDE) as u64;
            self.first_output.get_or_init(Instant::now);
            self.output_frames.fetch_add(frames, Ordering::Relaxed);
            self.out_period.store(frames, Ordering::Relaxed);
            let keep = self.in_period.load(Ordering::Relaxed) + MARGIN_FRAMES;

            let mut ring = self.ring.lock().expect("ring lock");
            let mut level = (ring.len() / CHANNELS) as u64;
            self.wait_min.fetch_min(level, Ordering::Relaxed);
            self.wait_max.fetch_max(level, Ordering::Relaxed);
            let primed = self.primed.load(Ordering::Relaxed);
            if (!primed && level >= keep) || level > DROP_ABOVE_FRAMES {
                let drop = level.saturating_sub(keep);
                ring.drain(..drop as usize * CHANNELS);
                level -= drop;
                if primed {
                    self.drop_frames.fetch_add(drop, Ordering::Relaxed);
                }
                self.primed.store(true, Ordering::Relaxed);
            }
            let take = if self.primed.load(Ordering::Relaxed) {
                level.min(frames)
            } else {
                0
            };
            let mut written = 0;
            for (dst, sample) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(ring.drain(..take as usize * CHANNELS))
            {
                *dst = sample.to_le_bytes();
                written += 4;
            }
            drop(ring);
            out[written..].fill(0);
            if take < frames && self.primed.load(Ordering::Relaxed) {
                // Short: the rest is silence, counted as a gap, and the next cycle primes again.
                self.gap_frames.fetch_add(frames - take, Ordering::Relaxed);
                self.primed.store(false, Ordering::Relaxed);
            }
        }

        fn print(&self) {
            let ppm = |first: &OnceLock<Instant>, frames: &AtomicU64| match first.get() {
                Some(at) => {
                    let expected = at.elapsed().as_secs_f64() * f64::from(RATE);
                    let frames = frames.load(Ordering::Relaxed) as f64;
                    format!("{:+.0}", (frames / expected - 1.0) * 1e6)
                }
                None => "-".to_owned(),
            };
            let ms = |frames: u64| frames as f64 * 1000.0 / f64::from(RATE);
            let wait_min = self.wait_min.swap(u64::MAX, Ordering::Relaxed);
            let wait_min = if wait_min == u64::MAX { 0 } else { wait_min };
            say!(
                "[probe] delivered_ppm={} played_ppm={} gap_ms={:.1} drop_ms={:.1} wait_ms={:.1} \
                 wait_max_ms={:.1} in_period={} out_period={}",
                ppm(&self.first_input, &self.input_frames),
                ppm(&self.first_output, &self.output_frames),
                ms(self.gap_frames.swap(0, Ordering::Relaxed)),
                ms(self.drop_frames.swap(0, Ordering::Relaxed)),
                ms(wait_min),
                ms(self.wait_max.swap(0, Ordering::Relaxed)),
                self.in_period.load(Ordering::Relaxed),
                self.out_period.load(Ordering::Relaxed),
            );
        }
    }

    pub fn main() {
        START.get_or_init(Instant::now);
        let args = args();
        pw::init();

        // The signal sources come first: spa blocks the signal in the calling thread when it adds
        // one (signalfd), and the threads started later inherit that mask, so the signal reaches
        // only this loop.
        let signals = pw::main_loop::MainLoopRc::new(None).expect("main loop");
        let (quit_tx, quit_rx) = pw::channel::channel::<()>();
        let (exit_tx, exit_rx) = pw::channel::channel::<()>();
        let signaled = Rc::new(Cell::new(None::<Instant>));
        let on_signal = {
            let (signals, signaled, quit_tx) = (signals.clone(), signaled.clone(), Rc::new(quit_tx));
            move || {
                if signaled.get().is_some() {
                    return;
                }
                say!("signal: closing");
                signaled.set(Some(Instant::now()));
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(1));
                    say!("closing took more than 1 s: exiting with 2");
                    std::process::exit(2);
                });
                let _ = quit_tx.send(());
                signals.quit();
            }
        };
        let _int = signals
            .loop_()
            .add_signal_local(pw::loop_::Signal::INT, on_signal.clone());
        let _term = signals.loop_().add_signal_local(pw::loop_::Signal::TERM, on_signal);
        // The pipewire thread ended on its own (an error): stop waiting for a signal.
        let _exit = exit_rx.attach(signals.loop_(), {
            let signals = signals.clone();
            move |()| signals.quit()
        });

        let worker = std::thread::Builder::new()
            .name("pipewire".into())
            .spawn(move || {
                let code = match run(&args, quit_rx) {
                    Ok(()) => 0,
                    Err(error) => {
                        say!("{error}");
                        1
                    }
                };
                let _ = exit_tx.send(());
                code
            })
            .expect("pipewire thread");

        signals.run();
        let code = worker.join().unwrap_or(3);
        if let Some(at) = signaled.get() {
            say!("closed in {} ms", at.elapsed().as_millis());
        }
        std::process::exit(code);
    }

    fn library_version() -> String {
        // SAFETY: PipeWire returns a static NUL-terminated string.
        let version = unsafe { std::ffi::CStr::from_ptr(pw::sys::pw_get_library_version()) };
        version.to_string_lossy().into_owned()
    }

    struct NodeInfo {
        name: String,
        description: String,
        class: String,
    }

    /// Runs the loop until the server has answered everything sent so far.
    fn roundtrip(mainloop: &pw::main_loop::MainLoopRc, core: &pw::core::CoreRc) -> Result<(), pw::Error> {
        let done = Rc::new(Cell::new(false));
        let pending = core.sync(0)?;
        let _listener = core
            .add_listener_local()
            .done({
                let (done, mainloop) = (done.clone(), mainloop.clone());
                move |id, seq| {
                    if id == pw::core::PW_ID_CORE && seq == pending {
                        done.set(true);
                        mainloop.quit();
                    }
                }
            })
            .register();
        while !done.get() {
            mainloop.run();
        }
        Ok(())
    }

    /// The `EnumFormat` both streams offer: F32 interleaved, FL FR, 48 kHz, nothing left open.
    fn format_pod() -> Vec<u8> {
        let mut info = AudioInfoRaw::new();
        info.set_format(AudioFormat::F32LE);
        info.set_rate(RATE);
        info.set_channels(CHANNELS as u32);
        let mut position = [0; MAX_CHANNELS];
        position[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
        position[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
        info.set_position(position);
        let object = spa::pod::Object {
            type_: spa::sys::SPA_TYPE_OBJECT_Format,
            id: spa::sys::SPA_PARAM_EnumFormat,
            properties: info.into(),
        };
        spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &spa::pod::Value::Object(object),
        )
        .expect("format pod")
        .0
        .into_inner()
    }

    fn run(args: &Args, quit_rx: pw::channel::Receiver<()>) -> Result<(), String> {
        let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(|e| format!("main loop: {e}"))?;
        let context = pw::context::ContextRc::new(&mainloop, None).map_err(|e| format!("context: {e}"))?;
        let core = context.connect_rc(None).map_err(|e| format!("connect: {e}"))?;
        let registry = core.get_registry_rc().map_err(|e| format!("registry: {e}"))?;

        // The nodes, and the default sink from the `default` metadata.
        let nodes: Rc<RefCell<Vec<NodeInfo>>> = Rc::default();
        let default_sink: Rc<RefCell<Option<String>>> = Rc::default();
        let metadata: Rc<RefCell<Vec<Box<dyn std::any::Any>>>> = Rc::default();
        let _registry_listener = registry
            .add_listener_local()
            .global({
                let (nodes, default_sink, metadata) = (nodes.clone(), default_sink.clone(), metadata.clone());
                let registry = registry.downgrade();
                move |global| {
                    let Some(props) = global.props else { return };
                    match global.type_ {
                        ObjectType::Node => nodes.borrow_mut().push(NodeInfo {
                            name: props.get("node.name").unwrap_or_default().to_owned(),
                            description: props.get("node.description").unwrap_or_default().to_owned(),
                            class: props.get("media.class").unwrap_or_default().to_owned(),
                        }),
                        ObjectType::Metadata if props.get("metadata.name") == Some("default") => {
                            let Some(registry) = registry.upgrade() else { return };
                            let Ok(proxy) = registry.bind::<pw::metadata::Metadata, _>(global) else {
                                return;
                            };
                            let sink = default_sink.clone();
                            let listener = proxy
                                .add_listener_local()
                                .property(move |_, key, _, value| {
                                    if key == Some("default.audio.sink") {
                                        *sink.borrow_mut() = value.map(str::to_owned);
                                    }
                                    0
                                })
                                .register();
                            metadata.borrow_mut().push(Box::new((proxy, listener)));
                        }
                        _ => {}
                    }
                }
            })
            .register();
        // Once for the globals, once for the metadata's properties.
        roundtrip(&mainloop, &core).map_err(|e| format!("sync: {e}"))?;
        roundtrip(&mainloop, &core).map_err(|e| format!("sync: {e}"))?;
        say!("PipeWire library {}", library_version());
        say!("default sink: {}", default_sink.borrow().as_deref().unwrap_or("(none)"));

        let input = {
            let nodes = nodes.borrow();
            let sources = nodes.iter().filter(|n| n.class == "Audio/Source");
            let wanted = args.input.to_lowercase();
            match sources.clone().find(|n| n.description.to_lowercase().contains(&wanted)) {
                Some(node) => node.name.clone(),
                None => {
                    say!(
                        "no Audio/Source whose description contains {:?}; the sources are:",
                        args.input
                    );
                    for node in sources {
                        say!("  {} ({})", node.description, node.name);
                    }
                    return Err("input not found".into());
                }
            }
        };
        say!("input: {input}");

        let latency = format!("{}/{RATE}", args.latency);
        let shared = Arc::new(Shared::new());
        let pod = format_pod();

        let capture = pw::stream::StreamRc::new(
            core.clone(),
            "capturax-probe-capture",
            properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Game",
                *pw::keys::NODE_LATENCY => latency.as_str(),
                *pw::keys::NODE_GROUP => GROUP,
                *pw::keys::TARGET_OBJECT => input.as_str(),
            },
        )
        .map_err(|e| format!("capture stream: {e}"))?;
        let _capture_listener = capture
            .add_local_listener_with_user_data(())
            .state_changed(|_, _, old, new| say!("capture: {old:?} -> {new:?}"))
            .param_changed(|_, _, id, param| log_format("capture", id, param))
            .process({
                let shared = shared.clone();
                move |stream, _| {
                    let Some(mut buffer) = stream.dequeue_buffer() else {
                        return;
                    };
                    let Some(data) = buffer.datas_mut().first_mut() else {
                        return;
                    };
                    let (offset, size) = (data.chunk().offset() as usize, data.chunk().size() as usize);
                    let Some(bytes) = data.data() else { return };
                    let end = (offset + size).min(bytes.len());
                    shared.push(&bytes[offset.min(end)..end]);
                }
            })
            .register()
            .map_err(|e| format!("capture listener: {e}"))?;

        let playback = pw::stream::StreamRc::new(
            core.clone(),
            "capturax-probe-playback",
            properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_CATEGORY => "Playback",
                *pw::keys::MEDIA_ROLE => "Game",
                *pw::keys::NODE_LATENCY => latency.as_str(),
                *pw::keys::NODE_GROUP => GROUP,
            },
        )
        .map_err(|e| format!("playback stream: {e}"))?;
        let _playback_listener = playback
            .add_local_listener_with_user_data(())
            .state_changed(|_, _, old, new| say!("playback: {old:?} -> {new:?}"))
            .param_changed(|_, _, id, param| log_format("playback", id, param))
            .process({
                let shared = shared.clone();
                move |stream, _| {
                    let Some(mut buffer) = stream.dequeue_buffer() else {
                        return;
                    };
                    let requested = buffer.requested() as usize;
                    let Some(data) = buffer.datas_mut().first_mut() else {
                        return;
                    };
                    let size = {
                        let Some(bytes) = data.data() else { return };
                        let room = bytes.len() / STRIDE;
                        let frames = if requested == 0 { room } else { requested.min(room) };
                        shared.pull(&mut bytes[..frames * STRIDE]);
                        frames * STRIDE
                    };
                    let chunk = data.chunk_mut();
                    *chunk.offset_mut() = 0;
                    *chunk.stride_mut() = STRIDE as i32;
                    *chunk.size_mut() = size as u32;
                }
            })
            .register()
            .map_err(|e| format!("playback listener: {e}"))?;

        let flags = pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS;
        let mut params = [Pod::from_bytes(&pod).expect("format pod")];
        capture
            .connect(spa::utils::Direction::Input, None, flags, &mut params)
            .map_err(|e| format!("capture connect: {e}"))?;
        let mut params = [Pod::from_bytes(&pod).expect("format pod")];
        playback
            .connect(spa::utils::Direction::Output, None, flags, &mut params)
            .map_err(|e| format!("playback connect: {e}"))?;

        let stats = mainloop.loop_().add_timer({
            let shared = shared.clone();
            move |_| shared.print()
        });
        stats.update_timer(Some(Duration::from_secs(1)), Some(Duration::from_secs(1)));

        let _quit = quit_rx.attach(mainloop.loop_(), {
            let (mainloop, capture, playback) = (mainloop.clone(), capture.clone(), playback.clone());
            move |()| {
                for (name, stream) in [("capture", &capture), ("playback", &playback)] {
                    if let Err(error) = stream.disconnect() {
                        say!("{name} disconnect: {error}");
                    }
                }
                mainloop.quit();
            }
        });

        mainloop.run();
        say!("loop quit");
        Ok(())
    }

    /// Prints the format PipeWire settled on (quickstart step 3 checks it with `pw-dump` too).
    fn log_format(stream: &str, id: u32, param: Option<&Pod>) {
        let Some(param) = param else { return };
        if id != spa::param::ParamType::Format.as_raw() {
            return;
        }
        match format_utils::parse_format(param) {
            Ok((MediaType::Audio, MediaSubtype::Raw)) => {
                let mut info = AudioInfoRaw::new();
                if info.parse(param).is_ok() {
                    say!(
                        "{stream}: format {:?} {} Hz {} channels {:?}",
                        info.format(),
                        info.rate(),
                        info.channels(),
                        &info.position()[..info.channels() as usize]
                    );
                }
            }
            _ => say!("{stream}: format is not raw audio"),
        }
    }
}
