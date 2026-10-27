//! Audio inputs and outputs from GStreamer's device monitor (research R7).

use gst::prelude::*;

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
        let api = props.get::<String>("device.api").unwrap_or_default();
        let loopback = props.get::<bool>("wasapi2.device.loopback").unwrap_or(false);
        let name = device.display_name().to_string();
        if api != preferred_api() || loopback || name.starts_with("Default Audio") {
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
/// for "ezcap Live GAMER ULTRA".
pub fn paired_input(inputs: Vec<AudioDevice>, video_name: &str) -> Option<AudioDevice> {
    let lower = video_name.to_lowercase();
    inputs.into_iter().find(|d| d.name.to_lowercase().contains(&lower))
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
    pub output: gst::Element,
}

/// Builds `input ! audioconvert ! audioresample ! queue ! output`; `output` None means the system
/// default.
pub fn build_preview(input: &AudioDevice, output: Option<&AudioDevice>, options: &PreviewOptions) -> anyhow::Result<Preview> {
    let src = input.device.create_element(Some("input"))?;
    let sink = match output {
        Some(d) => d.device.create_element(Some("output"))?,
        None => gst::ElementFactory::make(if cfg!(windows) { "wasapi2sink" } else { "pipewiresink" })
            .name("output")
            .build()?,
    };
    // low-latency picks WASAPI's own sizes, so it is left off the output when its buffer is set by hand.
    for (element, by_hand) in [(&src, false), (&sink, options.buffer_ms.is_some())] {
        if !by_hand && element.find_property("low-latency").is_some() {
            element.set_property("low-latency", true);
        }
    }
    if let Some(buffer) = options.buffer_ms {
        let us = buffer as i64 * 1000;
        sink.set_property("buffer-time", us);
        sink.set_property("latency-time", (us / 2).min(10_000));
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
    pipeline.add_many([&src, &convert, &resample, &queue, &sink])?;
    gst::Element::link_many([&src, &convert, &resample, &queue, &sink])?;
    Ok(Preview { pipeline, input: src, output: sink })
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
