//! The capture source: device element plus caps for the chosen native mode (research R6).

use gst::prelude::*;
use crate::devices::VideoDevice;
use crate::mode::{self, Mode};

pub struct Source {
    /// Ends with raw video in the device's native format; link it to the rest of the pipeline.
    pub bin: gst::Bin,
    pub mode: Mode,
}

/// Resolves the requested mode (or Auto) against the device's real modes.
pub fn choose_mode(device: &VideoDevice, requested: Option<&Mode>) -> anyhow::Result<Mode> {
    match requested {
        Some(request) => mode::find(&device.modes, request)
            .ok_or_else(|| anyhow::anyhow!("{} has no mode {request} (use --list)", device.name)),
        None => mode::auto(&device.modes).ok_or_else(|| anyhow::anyhow!("{} reports no modes", device.name)),
    }
}

/// Color range of the video levels.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum Range {
    /// 0-255
    Full,
    /// 16-235
    Limited,
}

/// Builds `device ! capsfilter [! jpeg decoder] [! capssetter]` with a ghost src pad; `range`
/// replaces the color range the device declares.
pub fn build(device: &VideoDevice, mode: Mode, range: Option<Range>) -> anyhow::Result<Source> {
    let bin = gst::Bin::with_name("capture");
    let src = device.device.create_element(Some("source"))?;
    let filter = gst::ElementFactory::make("capsfilter").property("caps", mode.caps()).build()?;
    bin.add_many([&src, &filter])?;
    src.link(&filter)?;

    let mut last = filter;
    if mode.format == "MJPG" {
        // GPU decoders first (research R6); the ezcap offers no MJPEG modes, so this is a fallback.
        let decoder = ["nvjpegdec", "d3d12mjpegdec", "jpegdec"]
            .iter()
            .find_map(|name| gst::ElementFactory::make(name).build().ok())
            .ok_or_else(|| anyhow::anyhow!("no JPEG decoder available"))?;
        bin.add(&decoder)?;
        last.link(&decoder)?;
        last = decoder;
    }

    if let Some(range) = range {
        // GStreamer colorimetry "range:matrix:transfer:primaries": BT.709 with the chosen range.
        let colorimetry = match range {
            Range::Full => "1:3:5:1",
            Range::Limited => "2:3:5:1",
        };
        let caps = gst::Caps::builder("video/x-raw").field("colorimetry", colorimetry).build();
        let setter = gst::ElementFactory::make("capssetter").property("caps", caps).build()?;
        bin.add(&setter)?;
        last.link(&setter)?;
        last = setter;
    }

    let pad = last.static_pad("src").expect("last element has a src pad");
    bin.add_pad(&gst::GhostPad::with_target(&pad)?)?;
    Ok(Source { bin, mode })
}
