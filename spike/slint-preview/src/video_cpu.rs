//! Frames reach Slint through CPU memory (research R5, variant 1), as in Slint's gstreamer-player
//! example: GPU color conversion when available, then `appsink` → RGBA buffer → `slint::Image`.

use gst::prelude::*;
use gst_video::video_frame::VideoFrameExt;

/// Elements between the capture source and the appsink: convert NV12/YUY2 to RGBA.
/// On Windows the conversion runs on the GPU (D3D12) and only the result is downloaded.
pub fn converter() -> anyhow::Result<gst::Bin> {
    let names: &[&str] = if cfg!(windows) && gst::ElementFactory::find("d3d12convert").is_some() {
        &["d3d12upload", "d3d12convert", "d3d12download"]
    } else {
        &["videoconvert"]
    };
    let bin = gst::Bin::with_name("to-rgba");
    let elements = names.iter().map(|n| gst::ElementFactory::make(n).build()).collect::<Result<Vec<_>, _>>()?;
    bin.add_many(&elements)?;
    gst::Element::link_many(&elements)?;
    let sink_pad = elements.first().unwrap().static_pad("sink").unwrap();
    let src_pad = elements.last().unwrap().static_pad("src").unwrap();
    bin.add_pad(&gst::GhostPad::with_target(&sink_pad)?)?;
    bin.add_pad(&gst::GhostPad::with_target(&src_pad)?)?;
    Ok(bin)
}

/// An appsink that keeps only the newest frame and never waits for timestamps.
/// `on_frame` runs on a GStreamer thread with each frame as an RGBA pixel buffer.
pub fn appsink(
    on_frame: impl Fn(slint::SharedPixelBuffer<slint::Rgba8Pixel>) + Send + Sync + 'static,
) -> gst_app::AppSink {
    let caps = gst_video::VideoCapsBuilder::new().format(gst_video::VideoFormat::Rgba).build();
    let sink = gst_app::AppSink::builder().caps(&caps).sync(false).max_buffers(1).drop(true).build();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer_owned().ok_or(gst::FlowError::Error)?;
                let info = sample
                    .caps()
                    .and_then(|c| gst_video::VideoInfo::from_caps(c).ok())
                    .ok_or(gst::FlowError::NotNegotiated)?;
                let frame = gst_video::VideoFrame::from_buffer_readable(buffer, &info)
                    .map_err(|_| gst::FlowError::Error)?;
                on_frame(to_pixels(&frame));
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    sink
}

/// Copies an RGBA frame row by row (rows may be padded) into a Slint pixel buffer.
fn to_pixels(frame: &gst_video::VideoFrame<gst_video::video_frame::Readable>) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
    let (width, height) = (frame.width(), frame.height());
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    let stride = frame.plane_stride()[0] as usize;
    let row = width as usize * 4;
    let data = frame.plane_data(0).expect("RGBA has one plane");
    let out = pixels.make_mut_bytes();
    for y in 0..height as usize {
        out[y * row..(y + 1) * row].copy_from_slice(&data[y * stride..y * stride + row]);
    }
    pixels
}
