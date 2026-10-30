//! Audio inputs and outputs from GStreamer's device monitor (research R7).

use gst::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct AudioDevice {
    pub name: String,
    pub device: gst::Device,
}

/// The audio API to use on this platform: WASAPI on Windows, PipeWire on Linux.
fn preferred_api() -> &'static str {
    if cfg!(windows) { "wasapi2" } else { "pipewire" }
}

/// Lists audio inputs (`Audio/Source`) or outputs (`Audio/Sink`), without loopback captures of
/// outputs and without the "Default ..." entries (no `--output` means the system default).
pub fn audio_devices(class: &str) -> anyhow::Result<Vec<AudioDevice>> {
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some(class), None);
    monitor.start()?;
    let devices = monitor.devices();
    monitor.stop();

    let mut found = Vec::new();
    for device in devices {
        let Some(props) = device.properties() else { continue };
        // PipeWire nodes report the API under them (e.g. "alsa"), so on Linux the device's type
        // tells whether it comes from the PipeWire provider.
        let api_ok = if cfg!(windows) {
            props.get::<String>("device.api").unwrap_or_default() == preferred_api()
        } else {
            device.type_().name() == "GstPipeWireDevice"
        };
        let loopback = props.get::<bool>("wasapi2.device.loopback").unwrap_or(false);
        let name = device.display_name().to_string();
        if !api_ok || loopback || name.starts_with("Default Audio") {
            continue;
        }
        found.push(AudioDevice { name, device });
    }
    Ok(found)
}

/// Finds a device by index or by part of its name (case-insensitive).
pub fn pick(devices: Vec<AudioDevice>, wanted: &str) -> anyhow::Result<AudioDevice> {
    if let Ok(index) = wanted.parse::<usize>() {
        return devices.into_iter().nth(index).ok_or_else(|| anyhow::anyhow!("no audio device at index {index}"));
    }
    let lower = wanted.to_lowercase();
    devices
        .into_iter()
        .find(|d| d.name.to_lowercase().contains(&lower))
        .ok_or_else(|| anyhow::anyhow!("no audio device named like {wanted:?} (use --list-audio)"))
}

/// The input whose name contains the video device's name, e.g. "HDMI (ezcap Live GAMER ULTRA)"
/// for "ezcap Live GAMER ULTRA". V4L2 appends the bus name after a colon ("ezcap Live GAMER
/// ULTRA: ezcap L"), which is left out.
pub fn paired_input(inputs: Vec<AudioDevice>, video_name: &str) -> Option<AudioDevice> {
    inputs.into_iter().find(|d| pairs(&d.name, video_name))
}

/// Whether an audio input's name belongs to the video device (see `paired_input`).
pub fn pairs(input_name: &str, video_name: &str) -> bool {
    let card = video_name.split(':').next().unwrap_or(video_name).trim();
    !card.is_empty() && input_name.to_lowercase().contains(&card.to_lowercase())
}

/// How the audio preview plays: see the audio-preview options in contracts/cli.md.
pub struct PreviewOptions {
    pub free_run: bool,
    pub buffer_ms: Option<u64>,
    pub max_queue_ms: u64,
    pub exclusive_input: bool,
    pub exclusive_output: bool,
}

impl PreviewOptions {
    /// The settings chosen in the spike: free-running, 20 ms output buffer, 20 ms queue.
    pub fn chosen() -> Self {
        Self { free_run: true, buffer_ms: Some(20), max_queue_ms: 20, exclusive_input: false, exclusive_output: false }
    }
}

pub struct Preview {
    pub pipeline: gst::Pipeline,
    pub input: gst::Element,
    pub queue: gst::Element,
    pub output: gst::Element,
}

/// The input element. On Linux it is `pulsesrc` (through pipewire-pulse), aimed at the PipeWire
/// node by name: next to `pulsesink`, `pipewiresrc` delivered only 17–25 % of the audio
/// (findings.md "Linux").
fn input_element(input: &AudioDevice) -> anyhow::Result<gst::Element> {
    if cfg!(windows) {
        return Ok(input.device.create_element(Some("input"))?);
    }
    pulse_element("pulsesrc", "input", Some(input))
}

/// The output element. On Linux it is `pulsesink`, aimed at the chosen PipeWire node by name:
/// `pipewiresink` hangs on stop in 4–7 of 10 runs (findings.md "Linux").
fn output_element(output: Option<&AudioDevice>) -> anyhow::Result<gst::Element> {
    if cfg!(windows) {
        return Ok(match output {
            Some(d) => d.device.create_element(Some("output"))?,
            None => gst::ElementFactory::make("wasapi2sink").name("output").build()?,
        });
    }
    pulse_element("pulsesink", "output", output)
}

/// A PulseAudio element on the PipeWire node of `device` (pipewire-pulse names it the same), or
/// on the system default.
fn pulse_element(factory: &str, name: &str, device: Option<&AudioDevice>) -> anyhow::Result<gst::Element> {
    let element = gst::ElementFactory::make(factory).name(name).build()?;
    if let Some(node) = device.and_then(|d| d.device.properties()).and_then(|p| p.get::<String>("node.name").ok()) {
        element.set_property("device", node);
    }
    Ok(element)
}

/// Builds `input ! audioconvert ! audioresample ! queue ! output`; `output` None means the system
/// default.
pub fn build_preview(input: &AudioDevice, output: Option<&AudioDevice>, options: &PreviewOptions) -> anyhow::Result<Preview> {
    let src = input_element(input)?;
    let sink = output_element(output)?;
    // low-latency picks WASAPI's own sizes, so it is left off the output when its buffer is set by hand.
    for (element, by_hand) in [(&src, false), (&sink, options.buffer_ms.is_some())] {
        if !by_hand && element.find_property("low-latency").is_some() {
            element.set_property("low-latency", true);
        }
    }
    if let Some(buffer) = options.buffer_ms {
        let us = buffer as i64 * 1000;
        let period_us = (us / 2).min(10_000);
        // On Linux the input gets the same sizes (pulsesrc reads in blocks of latency-time).
        let sized: &[&gst::Element] = if cfg!(windows) { &[&sink] } else { &[&src, &sink] };
        for element in sized {
            if element.find_property("buffer-time").is_some() {
                element.set_property("buffer-time", us);
                element.set_property("latency-time", period_us);
            }
        }
    }
    if options.free_run {
        sink.set_property("sync", false);
    }
    if options.exclusive_input || options.exclusive_output {
        if cfg!(not(windows)) {
            anyhow::bail!("exclusive mode is only available on Windows");
        }
        src.set_property("exclusive", options.exclusive_input);
        sink.set_property("exclusive", options.exclusive_output);
    }

    // On Linux the input is held to the device's own format (pipewiresrc negotiated mono, left
    // channel only, without it).
    let format = gst::ElementFactory::make("capsfilter")
        .property("caps", input.device.caps().filter(|_| cfg!(not(windows))).unwrap_or_else(gst::Caps::new_any))
        .build()?;
    let convert = gst::ElementFactory::make("audioconvert").build()?;
    let resample = gst::ElementFactory::make("audioresample").build()?;
    // Free-running, any delay at start-up stays in the buffers for good, since input and output
    // run at the same rate. A queue that drops its oldest audio lets the input drain and bounds
    // what waits; without free-run it holds nothing back (the clock paces the output).
    let queue = gst::ElementFactory::make("queue")
        .property("max-size-buffers", 0u32)
        .property("max-size-bytes", 0u32)
        .property("max-size-time", if options.free_run { options.max_queue_ms * 1_000_000 } else { 0u64 })
        .property_from_str("leaky", if options.free_run { "downstream" } else { "no" })
        .build()?;
    let pipeline = gst::Pipeline::with_name("audio-preview");
    pipeline.add_many([&src, &format, &convert, &resample, &queue, &sink])?;
    gst::Element::link_many([&src, &format, &convert, &resample, &queue, &sink])?;
    Ok(Preview { pipeline, input: src, queue, output: sink })
}

/// Nanoseconds of audio in a buffer, from the pad's caps (raw audio only).
fn audio_ns(pad: &gst::Pad, buffer: &gst::BufferRef) -> u64 {
    let Some(caps) = pad.current_caps() else { return 0 };
    let Some(s) = caps.structure(0) else { return 0 };
    let (Ok(rate), Ok(channels), Ok(format)) = (s.get::<i32>("rate"), s.get::<i32>("channels"), s.get::<&str>("format")) else {
        return 0;
    };
    let bytes = match format {
        f if f.starts_with("S8") || f.starts_with("U8") => 1,
        f if f.starts_with("S16") || f.starts_with("U16") => 2,
        f if f.starts_with("S24") && !f.starts_with("S24_32") => 3,
        f if f.starts_with("F64") => 8,
        _ => 4,
    };
    let frames = buffer.size() as u64 / (bytes * channels.max(1) as u64);
    frames * 1_000_000_000 / rate.max(1) as u64
}

/// Counts the audio entering the queue (as fast as the input delivers it) and entering the
/// output (as fast as the output takes it, once its buffer is full) against the system clock.
/// Diagnoses delay growth (T053): if both run at the same rate and nothing is dropped, the
/// delay is not growing inside the app.
pub struct DriftReporter {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DriftReporter {
    pub fn start(preview: &Preview, every: Duration) -> Self {
        let counted = |pad: gst::Pad| {
            let total = Arc::new(AtomicU64::new(0));
            let sum = total.clone();
            pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
                if let Some(buffer) = info.buffer() {
                    sum.fetch_add(audio_ns(pad, buffer), Ordering::Relaxed);
                }
                gst::PadProbeReturn::Ok
            });
            total
        };
        let input = counted(preview.queue.static_pad("sink").expect("queue has a sink pad"));
        let output = counted(preview.output.static_pad("sink").expect("output has a sink pad"));
        let queue = preview.queue.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            // Counting starts once audio flows, so start-up does not count as drift.
            while input.load(Ordering::Relaxed) == 0 && !flag.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_secs(2));
            let (start, in0, out0) = (Instant::now(), input.load(Ordering::Relaxed), output.load(Ordering::Relaxed));
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(every);
                let wall = start.elapsed().as_nanos() as f64;
                let got = (input.load(Ordering::Relaxed) - in0) as f64;
                let given = (output.load(Ordering::Relaxed) - out0) as f64;
                let level = queue.property::<u64>("current-level-time") as f64;
                println!(
                    "[drift] {:>5.0} s | vs system clock: input {:+5.0} ppm, output {:+5.0} ppm | dropped {:.1} ms | queue {:.1} ms",
                    wall / 1e9,
                    (got / wall - 1.0) * 1e6,
                    (given / wall - 1.0) * 1e6,
                    (got - given - level) / 1e6,
                    level / 1e6,
                );
            }
        });
        Self { stop, thread: Some(thread) }
    }
}

impl Drop for DriftReporter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn print_list(inputs: &[AudioDevice], outputs: &[AudioDevice]) {
    println!("Inputs:");
    for (index, d) in inputs.iter().enumerate() {
        println!("  [{index}] {}", d.name);
    }
    println!("Outputs:");
    for (index, d) in outputs.iter().enumerate() {
        println!("  [{index}] {}", d.name);
    }
}
