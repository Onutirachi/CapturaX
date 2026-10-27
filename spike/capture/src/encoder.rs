//! Encoder load test (research R8, T013): a branch that encodes the video as a recording would
//! and discards the result.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gst::prelude::*;

/// H.264 encoders in order of preference, with the properties for about 12 Mbps.
const CANDIDATES: &[(&str, &[(&str, u32)])] = &[
    ("nvh264enc", &[("bitrate", 12_000)]),
    ("nvd3d11h264enc", &[("bitrate", 12_000)]),
    ("d3d12h264enc", &[("bitrate", 12_000)]),
    ("amfh264enc", &[("bitrate", 12_000)]),
    ("qsvh264enc", &[("bitrate", 12_000)]),
    ("mfh264enc", &[("bitrate", 12_000)]),
    ("vah264enc", &[("bitrate", 12_000)]),
    ("x264enc", &[("bitrate", 12_000)]),
    ("openh264enc", &[("bitrate", 12_000_000)]),
];

pub struct EncoderBranch {
    /// `queue ! videoconvert ! encoder ! fakesink` with a ghost sink pad.
    pub bin: gst::Bin,
    /// Factory name of the encoder in use.
    pub name: String,
    frames: Arc<AtomicU64>,
}

/// Builds the branch with the first available encoder.
pub fn build() -> anyhow::Result<EncoderBranch> {
    let (name, props) = CANDIDATES
        .iter()
        .find(|(name, _)| gst::ElementFactory::find(name).is_some())
        .ok_or_else(|| anyhow::anyhow!("no H.264 encoder available"))?;
    let mut encoder = gst::ElementFactory::make(name);
    for (prop, value) in *props {
        encoder = encoder.property(prop, value);
    }
    let encoder = encoder.build()?;
    // The queue is not leaky: a recording must not lose frames, so a slow encoder shows up as
    // back-pressure on the preview, which is what the test looks for.
    let queue = gst::ElementFactory::make("queue").build()?;
    let convert = gst::ElementFactory::make("videoconvert").build()?;
    let sink = gst::ElementFactory::make("fakesink").property("sync", false).build()?;

    let bin = gst::Bin::with_name("encoder");
    bin.add_many([&queue, &convert, &encoder, &sink])?;
    gst::Element::link_many([&queue, &convert, &encoder, &sink])?;
    let pad = queue.static_pad("sink").expect("queue has a sink pad");
    bin.add_pad(&gst::GhostPad::with_target(&pad)?)?;

    let frames = Arc::new(AtomicU64::new(0));
    let counter = frames.clone();
    sink.static_pad("sink").expect("fakesink has a sink pad").add_probe(gst::PadProbeType::BUFFER, move |_, _| {
        counter.fetch_add(1, Ordering::Relaxed);
        gst::PadProbeReturn::Ok
    });

    Ok(EncoderBranch { bin, name: name.to_string(), frames })
}

impl EncoderBranch {
    /// Prints the encoded frames per second until the process ends.
    pub fn report(&self) {
        let frames = self.frames.clone();
        std::thread::spawn(move || {
            let mut last = 0;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let now = frames.load(Ordering::Relaxed);
                println!("[enc]   {:>4} fps encoded", now - last);
                last = now;
            }
        });
    }
}
